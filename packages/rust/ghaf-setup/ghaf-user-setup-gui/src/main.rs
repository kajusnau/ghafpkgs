// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The Ghaf first-boot user setup: a libcosmic application that creates
//! the local account with homectl.

use std::any::TypeId;
use std::path::Path;
use std::sync::{Arc, Mutex};

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{Limits, Subscription};
use cosmic::{Application, Element, executor};
use ghaf_setup_core::SystemRunner;
use ghaf_setup_core::homed::{
    self, AccountConfig, RecoveryKey, create_account, fido_token_present,
};
use ghaf_setup_core::log_runner::LoggingRunner;
use ghaf_setup_core::pace::{MIN_STEP, Update, run};
use ghaf_setup_core::progress::ProgressSender;
use ghaf_setup_ui::{MAX_HEIGHT, MAX_WIDTH, Nav, Page as PageTrait, PageMessage, frame};
use ghaf_user_setup_gui::availability::{check, debounce};
use ghaf_user_setup_gui::page::{account, pages, running};
use ghaf_user_setup_gui::run_guard::RunGuard;
use indexmap::IndexMap;
use tokio::sync::mpsc;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // A centred card, not a fullscreen takeover. Same limits as
    // cosmic-initial-setup.
    let mut settings =
        Settings::default().size_limits(Limits::NONE.max_width(MAX_WIDTH).max_height(MAX_HEIGHT));
    if let Some(theme) = std::env::var("XDG_DATA_DIRS")
        .ok()
        .and_then(|dirs| ghaf_setup_ui::theme::load(&dirs))
    {
        settings = settings.theme(theme);
    }

    cosmic::app::run::<App>(settings, ())
}

// Debug is redacted: the account page's messages carry the password and
// PIN, and a run's result the recovery key; none may reach a log.
#[derive(Clone)]
enum Message {
    Page(PageMessage),
    Fido(bool),
    Availability {
        generation: u64,
        name: String,
        taken: Option<bool>,
    },
    Run(Update),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Message(..)")
    }
}

struct App {
    core: Core,
    config: AccountConfig,
    pages: IndexMap<TypeId, Box<dyn PageTrait>>,
    nav: Nav,
    /// Bumped on every username edit so only the latest check lands.
    check_generation: u64,
    /// The recovery key from the last run, taken when it finishes.
    key_slot: Arc<Mutex<Option<RecoveryKey>>>,
    progress_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Update>>>>,
    run_started: bool,
    run_guard: RunGuard,
    /// Bumped on every `spawn_create` so the subscription's hash changes
    /// and iced rebuilds its stream instead of reusing an exhausted one.
    run_generation: u64,
}

impl App {
    fn account_page_mut(&mut self) -> Option<&mut account::Page> {
        self.pages
            .get_mut(&TypeId::of::<account::Page>())
            .and_then(|p| p.as_any().downcast_mut::<account::Page>())
    }

    fn running_page_mut(&mut self) -> Option<&mut running::Page> {
        self.pages
            .get_mut(&TypeId::of::<running::Page>())
            .and_then(|p| p.as_any().downcast_mut::<running::Page>())
    }

    /// Spawns the account creation; its updates reach the running page
    /// through the subscription. The password leaves the form here.
    fn spawn_create(&mut self) {
        let (out, rx) = mpsc::unbounded_channel();
        self.run_generation += 1;
        *self.progress_rx.lock().unwrap() = Some(rx);
        self.run_started = true;

        if let Some(page) = self.running_page_mut() {
            page.reset();
        }

        let Some(page) = self.account_page_mut() else {
            return;
        };
        let request = page.request();
        page.clear_secrets();

        // A form that is not complete must still end with a result, not
        // leave the running page waiting forever.
        let Some(request) = request else {
            let _ = out.send(Update::Finished(Err(
                "the account details are incomplete".to_string()
            )));
            return;
        };

        if let Some(page) = self.running_page_mut() {
            page.set_fido(request.fido);
        }

        let config = self.config.clone();
        let key_slot = self.key_slot.clone();
        let work = move |tx: ProgressSender| async move {
            let runner = LoggingRunner::new(SystemRunner, tx.clone());
            let key = create_account(&runner, &config, &request, &tx)
                .await
                .map_err(|e| e.to_string())?;
            *key_slot.lock().unwrap() = key;
            Ok(())
        };
        tokio::spawn(run(work, out, MIN_STEP));
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "ae.tii.GhafUserSetup";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        core.window.show_close = false;
        core.window.show_maximize = false;
        core.window.show_minimize = false;

        let config = homed::load_config(Path::new(homed::CONFIG_PATH));
        let fido_auth = config.fido_auth;
        let page_set = pages(false);

        let app = App {
            core,
            config,
            nav: Nav::new(page_set.len()),
            pages: page_set,
            check_generation: 0,
            key_slot: Arc::new(Mutex::new(None)),
            progress_rx: Arc::new(Mutex::new(None)),
            run_started: false,
            run_guard: RunGuard::default(),
            run_generation: 0,
        };

        let task = if fido_auth {
            cosmic::Task::future(async { Message::Fido(fido_token_present(&SystemRunner).await) })
                .map(cosmic::Action::App)
        } else {
            Task::none()
        };
        (app, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Fido(present) => {
                if let Some(page) = self.account_page_mut() {
                    page.set_fido_available(present);
                }
            }

            Message::Availability {
                generation,
                name,
                taken,
            } => {
                if generation == self.check_generation
                    && let Some(taken) = taken
                    && let Some(page) = self.account_page_mut()
                {
                    page.set_username_taken(&name, taken);
                }
            }

            Message::Run(Update::Progress(event)) => {
                if let Some(page) = self.running_page_mut() {
                    page.apply(event);
                }
            }

            Message::Run(Update::Finished(result)) => {
                self.run_guard.finish();
                let result = result.map(|()| self.key_slot.lock().unwrap().take());
                if let Some(page) = self.running_page_mut() {
                    page.conclude(result);
                }
            }

            Message::Page(PageMessage::PageOpen(index)) => {
                self.nav.go_to(index);
                // A second open while homectl runs would lose its key.
                if index == 1 && self.run_guard.try_start() {
                    self.spawn_create();
                }
            }

            // The unit is a oneshot that greetd is ordered after, so a
            // clean exit is what lets the login screen start.
            Message::Page(PageMessage::Finish) => std::process::exit(0),

            Message::Page(PageMessage::App { page, payload }) => {
                if page == TypeId::of::<account::Page>() {
                    if let Some(msg) = payload.downcast_ref::<account::Message>().cloned()
                        && let Some(p) = self.account_page_mut()
                    {
                        let checks_name = matches!(msg, account::Message::Username(_));
                        p.update(msg);
                        // The page may have refused the edit, so check what it kept.
                        if checks_name {
                            let name = p.username().to_string();
                            self.check_generation += 1;
                            let generation = self.check_generation;
                            return cosmic::Task::future(async move {
                                tokio::time::sleep(debounce()).await;
                                let taken = check(&SystemRunner, &name).await;
                                Message::Availability {
                                    generation,
                                    name,
                                    taken,
                                }
                            })
                            .map(cosmic::Action::App);
                        }
                    }
                } else if page == TypeId::of::<running::Page>()
                    && let Some(msg) = payload.downcast_ref::<running::Message>().cloned()
                    && let Some(p) = self.running_page_mut()
                {
                    p.update(msg);
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
            rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Update>>>>,
            generation: u64,
        }

        impl std::hash::Hash for ProgressSource {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                "ghaf-user-setup-progress".hash(state);
                self.generation.hash(state);
            }
        }

        if !self.run_started {
            return Subscription::none();
        }

        let source = ProgressSource {
            rx: self.progress_rx.clone(),
            generation: self.run_generation,
        };
        Subscription::run_with(source, |source: &ProgressSource| {
            // Taken once and owned for the stream's lifetime, not taken and
            // put back per poll.
            let rx = source.rx.lock().unwrap().take();
            cosmic::iced::futures::stream::unfold(rx, |mut rx| async move {
                let event = rx.as_mut()?.recv().await?;
                Some((event, rx))
            })
        })
        .map(Message::Run)
    }
}
