// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The Ghaf installer wizard: a libcosmic application that wires the six
//! wizard pages together.

use std::any::{Any, TypeId};
use std::sync::{Arc, Mutex};

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{Limits, Subscription};
use cosmic::{Application, Element, executor, widget};
use ghaf_installer_gui::boot_device::{Unresolved, derive_boot_device};
use ghaf_installer_gui::page::{Mode, complete, confirm, disk, options, pages, running, welcome};
use ghaf_setup_core::SystemRunner;
use ghaf_setup_core::disk::BlockDevice;
use ghaf_setup_core::install::boot::{enroll_secureboot, in_setup_mode};
use ghaf_setup_core::install::{ImageSource, InstallRequest, install};
use ghaf_setup_core::progress::ProgressEvent;
use ghaf_setup_ui::{MAX_HEIGHT, MAX_WIDTH, Nav, Page as PageTrait, PageMessage, frame};
use indexmap::IndexMap;
use tokio::sync::mpsc;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // IMG_PATH is set by the installer image's systemd unit; a
    // ghaf.image_url= kernel parameter overrides it, exactly as the shell
    // installer resolves it (ghaf-installer-tui.sh:120).
    let img_path = resolve_img_path();

    // A centred card, not a fullscreen takeover. Same limits as
    // cosmic-initial-setup.
    let mut settings =
        Settings::default().size_limits(Limits::NONE.max_width(MAX_WIDTH).max_height(MAX_HEIGHT));
    if let Some(theme) = std::env::var("XDG_DATA_DIRS")
        .ok()
        .and_then(|dirs| ghaf_installer_gui::theme::load(&dirs))
    {
        settings = settings.theme(theme);
    }

    cosmic::app::run::<App>(settings, Flags { img_path })
}

/// Reads `IMG_PATH`, falling back to a `ghaf.image_url=` kernel parameter.
/// `Err` means neither was present, which the app must surface rather than
/// silently proceeding with a bogus source. Kept as the raw string (rather
/// than parsed into an `ImageSource` here) because `boot_device::derive_boot_device`
/// also needs the raw value to recognise the `http(s)://` netboot case.
fn resolve_img_path() -> Result<String, String> {
    if let Ok(path) = std::env::var("IMG_PATH") {
        return Ok(path);
    }

    let cmdline = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("ghaf.image_url="))
        .map(str::to_string)
        .ok_or_else(|| "IMG_PATH is not set".to_string())
}

/// Enumerating disks and checking Secure Boot Setup Mode both need a
/// subprocess; the real disk list and options page arrive via
/// `Message::Devices` once this completes.
fn load_devices(img_path: String) -> Task<Message> {
    cosmic::Task::future(async move {
        let (boot_device, boot_device_warning) =
            match derive_boot_device(&SystemRunner, &img_path).await {
                Ok(device) => (device, None),
                Err(Unresolved) => (
                    None,
                    Some(
                        "Could not identify the installer's own boot medium; it may still \
                         appear in the disk list below."
                            .to_string(),
                    ),
                ),
            };
        let devices = ghaf_setup_core::disk::enumerate(&SystemRunner, boot_device.as_deref())
            .await
            .unwrap_or_default();
        let setup_mode = in_setup_mode(&SystemRunner).await;
        Message::Devices {
            devices,
            setup_mode,
            boot_device_warning,
        }
    })
    .map(cosmic::Action::App)
}

struct Flags {
    img_path: Result<String, String>,
}

/// A terminal screen shown when the image source cannot be resolved at all.
/// Matches the shell installer's behaviour of refusing to proceed rather
/// than silently guessing a source.
struct FatalPage {
    message: String,
}

impl PageTrait for FatalPage {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        "Cannot start the installer".into()
    }

    // The only page in the wizard, so "Finish" is the sole way out; see the
    // Finish handler, which reboots from here.
    fn completed(&self) -> bool {
        true
    }

    fn finish_label(&self) -> String {
        "Reboot now".into()
    }

    fn view(&self) -> Element<'_, PageMessage> {
        widget::text::body(self.message.clone()).into()
    }
}

#[derive(Clone, Debug)]
enum Message {
    Page(PageMessage),
    Devices {
        devices: Vec<BlockDevice>,
        setup_mode: bool,
        boot_device_warning: Option<String>,
    },
    Progress(ProgressEvent),
    InstallFinished(Result<(), String>),
}

struct App {
    core: Core,
    mode: Mode,
    pages: IndexMap<TypeId, Box<dyn PageTrait>>,
    nav: Nav,
    image_source: Option<ImageSource>,
    /// The raw `IMG_PATH`, kept to re-read the disks when starting over.
    img_path: String,
    setup_mode: bool,
    progress_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ProgressEvent>>>>,
    install_started: bool,
    /// Bumped on every `spawn_install` so the subscription's hash changes
    /// and iced rebuilds its stream instead of reusing an exhausted one.
    install_generation: u64,
}

impl App {
    fn confirm_slot(&self) -> usize {
        if self.mode == Mode::Install { 3 } else { 2 }
    }

    fn running_page_mut(&mut self) -> Option<&mut running::Page> {
        self.pages
            .get_mut(&TypeId::of::<running::Page>())
            .and_then(|p| p.as_any().downcast_mut::<running::Page>())
    }

    /// Rebuilds the page set to match the mode chosen on the welcome page,
    /// adding or removing the options page as needed. The options page is
    /// meaningless when only erasing, so it must not linger if the user goes
    /// back and changes their mind.
    fn sync_mode(&mut self) {
        let chosen = self
            .pages
            .get_mut(&TypeId::of::<welcome::Page>())
            .and_then(|p| p.as_any().downcast_mut::<welcome::Page>())
            .and_then(|p| p.action());

        let desired = match chosen {
            Some(welcome::Action::Install) => Mode::Install,
            Some(welcome::Action::Erase) => Mode::Erase,
            None => return,
        };

        if desired == self.mode {
            return;
        }

        let current = self.nav.current();
        let options_key = TypeId::of::<options::Page>();

        match desired {
            Mode::Install => {
                if !self.pages.contains_key(&options_key) {
                    self.pages.shift_insert(
                        2,
                        options_key,
                        Box::new(options::Page::new(self.setup_mode)),
                    );
                }
            }
            Mode::Erase => {
                self.pages.shift_remove(&options_key);
            }
        }

        if let Some(page) = self.running_page_mut() {
            page.set_erase(desired == Mode::Erase);
        }
        self.mode = desired;
        self.nav = Nav::new(self.pages.len());
        self.nav.go_to(current);
    }

    /// Builds (or refreshes) the confirmation summary from earlier answers
    /// and makes sure the confirm page occupies its slot right before the
    /// running page. `IndexMap::insert` on an existing key replaces the
    /// value in place without moving it, so a refresh never needs the shift.
    fn sync_confirm_page(&mut self) {
        let device = self
            .pages
            .get_mut(&TypeId::of::<disk::Page>())
            .and_then(|p| p.as_any().downcast_mut::<disk::Page>())
            .and_then(|p| p.selected())
            .map(|d| d.path.clone())
            .unwrap_or_default();

        let (encrypt, secure_boot) = self
            .pages
            .get_mut(&TypeId::of::<options::Page>())
            .and_then(|p| p.as_any().downcast_mut::<options::Page>())
            .map(|p| (p.encrypt(), p.secure_boot()))
            .unwrap_or((false, false));

        let action = match self.mode {
            Mode::Install => "Install Ghaf",
            Mode::Erase => "Erase disk",
        };

        let summary = confirm::Summary {
            action: action.to_string(),
            wipe_only: self.mode == Mode::Erase,
            device,
            encrypt,
            secure_boot,
        };

        let key = TypeId::of::<confirm::Page>();
        if self.pages.contains_key(&key) {
            self.pages
                .insert(key, Box::new(confirm::Page::new(summary)));
        } else {
            let slot = self.confirm_slot();
            self.pages
                .shift_insert(slot, key, Box::new(confirm::Page::new(summary)));
            let current = self.nav.current();
            self.nav = Nav::new(self.pages.len());
            self.nav.go_to(current);
        }
    }

    fn install_request(&mut self) -> Option<InstallRequest> {
        let device = self
            .pages
            .get_mut(&TypeId::of::<disk::Page>())
            .and_then(|p| p.as_any().downcast_mut::<disk::Page>())
            .and_then(|p| p.selected())
            .map(|d| d.path.clone())?;

        let (encrypt, secure_boot) = self
            .pages
            .get_mut(&TypeId::of::<options::Page>())
            .and_then(|p| p.as_any().downcast_mut::<options::Page>())
            .map(|p| (p.encrypt(), p.secure_boot()))
            .unwrap_or((false, false));

        Some(InstallRequest {
            device,
            source: self.image_source.clone()?,
            encrypt,
            secure_boot,
            wipe_only: self.mode == Mode::Erase,
        })
    }

    /// Spawns the install (or, when `secure_boot_only` is set, just the
    /// Secure Boot enrollment retry) and bridges its progress channel into
    /// the subscription that feeds the running page.
    fn spawn_install(&mut self, secure_boot_only: bool) -> Task<Message> {
        if secure_boot_only {
            if let Some(page) = self.running_page_mut() {
                page.set_plan(vec![ghaf_setup_core::progress::Phase::SecureBoot]);
            }
            let (tx, rx) = mpsc::unbounded_channel();
            self.install_generation += 1;
            *self.progress_rx.lock().unwrap() = Some(rx);
            self.install_started = true;

            return cosmic::Task::future(async move {
                enroll_secureboot(&SystemRunner, &tx)
                    .await
                    .map_err(|e| e.to_string())
            })
            .map(Message::InstallFinished)
            .map(cosmic::Action::App);
        }

        // A missing device or image source must still land the wizard on a
        // result screen, not leave the running page stuck at 0% forever.
        let Some(request) = self.install_request() else {
            return cosmic::Task::future(async {
                Message::InstallFinished(Err("no disk or image source selected".to_string()))
            })
            .map(cosmic::Action::App);
        };

        if let Some(page) = self.running_page_mut() {
            page.set_plan(request.phases());
        }
        let (tx, rx) = mpsc::unbounded_channel();
        self.install_generation += 1;
        *self.progress_rx.lock().unwrap() = Some(rx);
        self.install_started = true;

        cosmic::Task::future(async move {
            install(&SystemRunner, &request, &tx)
                .await
                .map_err(|e| e.to_string())
        })
        .map(Message::InstallFinished)
        .map(cosmic::Action::App)
    }

    /// Returns to the start page with a fresh page set and a re-read disk
    /// list, e.g. after erasing a disk.
    fn start_over(&mut self) -> Task<Message> {
        self.mode = Mode::Install;
        self.pages = pages(Mode::Install, Vec::new(), self.setup_mode);
        self.nav = Nav::new(self.pages.len());
        self.install_started = false;
        *self.progress_rx.lock().unwrap() = None;
        load_devices(self.img_path.clone())
    }

    fn advance_to_complete(&mut self) -> Task<Message> {
        let state =
            self.running_page_mut()
                .map(|p| p.state())
                .unwrap_or(running::RunState::Failed {
                    recoverable: false,
                    disk_untouched: false,
                });

        let failure = self.running_page_mut().and_then(|p| p.failure());
        let device = self
            .pages
            .get_mut(&TypeId::of::<disk::Page>())
            .and_then(|p| p.as_any().downcast_mut::<disk::Page>())
            .and_then(|p| p.selected())
            .map(|d| d.path.clone())
            .unwrap_or_default();
        self.pages.insert(
            TypeId::of::<complete::Page>(),
            Box::new(complete::Page::new(
                state,
                self.mode == Mode::Erase,
                device,
                failure,
            )),
        );

        if let Some(index) = self.pages.get_index_of(&TypeId::of::<complete::Page>()) {
            self.nav.go_to(index);
        }

        Task::none()
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "ae.tii.GhafInstaller";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        core.window.show_close = false;
        core.window.show_maximize = false;
        core.window.show_minimize = false;

        match flags.img_path {
            Err(message) => {
                let mut pages: IndexMap<TypeId, Box<dyn PageTrait>> = IndexMap::new();
                pages.insert(TypeId::of::<FatalPage>(), Box::new(FatalPage { message }));

                let app = App {
                    core,
                    mode: Mode::Install,
                    nav: Nav::new(pages.len()),
                    pages,
                    image_source: None,
                    img_path: String::new(),
                    setup_mode: false,
                    progress_rx: Arc::new(Mutex::new(None)),
                    install_started: false,
                    install_generation: 0,
                };
                (app, Task::none())
            }
            Ok(img_path) => {
                let mode = Mode::Install;
                let page_set = pages(mode, Vec::new(), false);

                let app = App {
                    core,
                    mode,
                    nav: Nav::new(page_set.len()),
                    pages: page_set,
                    image_source: Some(ImageSource::parse(&img_path)),
                    img_path: img_path.clone(),
                    setup_mode: false,
                    progress_rx: Arc::new(Mutex::new(None)),
                    install_started: false,
                    install_generation: 0,
                };

                let load = load_devices(img_path);
                (app, load)
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Devices {
                devices,
                setup_mode,
                boot_device_warning,
            } => {
                self.setup_mode = setup_mode;

                let previous_selection = self
                    .pages
                    .get_mut(&TypeId::of::<disk::Page>())
                    .and_then(|p| p.as_any().downcast_mut::<disk::Page>())
                    .and_then(|p| p.selected())
                    .map(|d| d.path.clone());

                self.pages.insert(
                    TypeId::of::<disk::Page>(),
                    Box::new(disk::Page::new_with_selection(
                        devices,
                        previous_selection.as_deref(),
                        boot_device_warning,
                    )),
                );

                if let Some(p) = self
                    .pages
                    .get_mut(&TypeId::of::<options::Page>())
                    .and_then(|p| p.as_any().downcast_mut::<options::Page>())
                {
                    p.set_setup_mode(setup_mode);
                }
            }

            Message::Progress(event) => {
                if let Some(page) = self.running_page_mut() {
                    page.apply(event);
                }
            }

            Message::InstallFinished(result) => {
                if let Some(page) = self.running_page_mut() {
                    match result {
                        Ok(()) => page.finish(),
                        Err(message) => {
                            if matches!(page.state(), running::RunState::Running) {
                                let phase = page
                                    .last_phase()
                                    .unwrap_or(ghaf_setup_core::progress::Phase::Wipe);
                                page.apply(ProgressEvent::Failed {
                                    phase,
                                    message,
                                    recoverable: false,
                                });
                            }
                        }
                    }
                }
                return self.advance_to_complete();
            }

            Message::Page(PageMessage::PageOpen(index)) => {
                if self.nav.current() == 0 {
                    self.sync_mode();
                }
                if index == self.confirm_slot() {
                    self.sync_confirm_page();
                }

                let is_running = self
                    .pages
                    .get_index(index)
                    .map(|(key, _)| *key == TypeId::of::<running::Page>())
                    .unwrap_or(false);
                let is_destructive = self
                    .pages
                    .get_index(index)
                    .map(|(_, page)| page.destructive())
                    .unwrap_or(false);

                self.nav.go_to(index);

                if is_running && !self.install_started {
                    return self.spawn_install(false);
                }

                // A stray Enter must not fire the destructive primary
                // action: steer default focus to Back instead.
                if is_destructive {
                    return widget::button::focus(ghaf_setup_ui::back_button_id())
                        .map(cosmic::Action::App);
                }
            }

            Message::Page(PageMessage::Finish) => {
                let starts_over = self
                    .pages
                    .get_mut(&TypeId::of::<complete::Page>())
                    .and_then(|p| p.as_any().downcast_mut::<complete::Page>())
                    .is_some_and(|p| p.starts_over());
                let on_complete = self
                    .pages
                    .get_index(self.nav.current())
                    .is_some_and(|(key, _)| *key == TypeId::of::<complete::Page>());
                if on_complete && starts_over {
                    return self.start_over();
                }

                let reboots = self
                    .pages
                    .get_index(self.nav.current())
                    .map(|(key, _)| {
                        *key == TypeId::of::<complete::Page>() || *key == TypeId::of::<FatalPage>()
                    })
                    .unwrap_or(false);

                if reboots {
                    let _ = std::process::Command::new("systemctl")
                        .arg("reboot")
                        .status();
                }
            }

            Message::Page(PageMessage::App { page, payload }) => {
                if page == TypeId::of::<welcome::Page>() {
                    match payload.downcast_ref::<welcome::Message>() {
                        // A clean exit ends the unit, whose ExecStopPost hands tty1 to getty.
                        Some(welcome::Message::ExitToShell) => std::process::exit(0),
                        Some(welcome::Message::Restart) => {
                            let _ = std::process::Command::new("systemctl")
                                .arg("reboot")
                                .status();
                            return Task::none();
                        }
                        Some(welcome::Message::ShutDown) => {
                            let _ = std::process::Command::new("systemctl")
                                .arg("poweroff")
                                .status();
                            return Task::none();
                        }
                        _ => {}
                    }
                    if let Some(msg) = payload.downcast_ref::<welcome::Message>().cloned()
                        && let Some(p) = self
                            .pages
                            .get_mut(&page)
                            .and_then(|p| p.as_any().downcast_mut::<welcome::Page>())
                    {
                        p.update(msg);
                        self.sync_mode();
                        self.nav.go_to(1);
                    }
                } else if page == TypeId::of::<disk::Page>() {
                    if let Some(msg) = payload.downcast_ref::<disk::Message>().cloned()
                        && let Some(p) = self
                            .pages
                            .get_mut(&page)
                            .and_then(|p| p.as_any().downcast_mut::<disk::Page>())
                    {
                        p.update(msg);
                    }
                } else if page == TypeId::of::<options::Page>() {
                    if let Some(msg) = payload.downcast_ref::<options::Message>().cloned()
                        && let Some(p) = self
                            .pages
                            .get_mut(&page)
                            .and_then(|p| p.as_any().downcast_mut::<options::Page>())
                    {
                        p.update(msg);
                    }
                } else if page == TypeId::of::<complete::Page>() {
                    match payload.downcast_ref::<complete::Message>() {
                        Some(complete::Message::Restart) => {
                            let _ = std::process::Command::new("systemctl")
                                .arg("reboot")
                                .status();
                            return Task::none();
                        }
                        Some(complete::Message::ShutDown) => {
                            let _ = std::process::Command::new("systemctl")
                                .arg("poweroff")
                                .status();
                            return Task::none();
                        }
                        Some(complete::Message::Retry) => {}
                        None => return Task::none(),
                    }
                    // Retry is reachable only on `Failed { recoverable: true }`,
                    // which only a failed Secure Boot key enrollment produces
                    // (see running::RunState's doc comment) -- so retrying
                    // re-runs just that step, not the whole install.
                    if let Some(p) = self.running_page_mut() {
                        p.reset();
                    }
                    if let Some(index) = self.pages.get_index_of(&TypeId::of::<running::Page>()) {
                        self.nav.go_to(index);
                    }
                    return self.spawn_install(true);
                }
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let (_, page) = self
            .pages
            .get_index(self.nav.current())
            .expect("nav always points at a page that exists");
        frame(page.as_ref(), &self.nav).map(Message::Page)
    }

    fn subscription(&self) -> Subscription<Message> {
        #[derive(Clone)]
        struct ProgressSource {
            rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ProgressEvent>>>>,
            generation: u64,
        }

        impl std::hash::Hash for ProgressSource {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                "ghaf-installer-progress".hash(state);
                self.generation.hash(state);
            }
        }

        if !self.install_started {
            return Subscription::none();
        }

        let source = ProgressSource {
            rx: self.progress_rx.clone(),
            generation: self.install_generation,
        };
        Subscription::run_with(source, |source: &ProgressSource| {
            // Taken once and owned for the stream's lifetime, not taken and
            // put back per poll -- that froze progress mid-install.
            let rx = source.rx.lock().unwrap().take();
            cosmic::iced::futures::stream::unfold(rx, |mut rx| async move {
                let event = rx.as_mut()?.recv().await?;
                Some((event, rx))
            })
        })
        .map(Message::Progress)
    }
}
