// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The result of the install or erase, shown under the running checklist.

use crate::page::running::RunState;
use cosmic::iced::Length;
use cosmic::{Apply, Element, widget};
use ghaf_setup_core::progress::Phase;
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    Retry,
    Restart,
    ShutDown,
    /// The success page's Reboot: restarts after a countdown.
    Reboot,
    /// One second of the countdown has passed.
    Tick,
}

/// Seconds between pressing Reboot and the restart.
pub const REBOOT_DELAY: u32 = 10;

pub struct Page {
    state: RunState,
    erase: bool,
    device: String,
    failure: Option<(Phase, String)>,
    /// Seconds left before rebooting, once Reboot has been pressed.
    countdown: Option<u32>,
    /// Why the last reboot or shutdown did not happen.
    power_error: Option<String>,
}

impl Page {
    pub fn new(
        state: RunState,
        erase: bool,
        device: String,
        failure: Option<(Phase, String)>,
    ) -> Self {
        Self {
            state,
            erase,
            device,
            failure,
            countdown: None,
            power_error: None,
        }
    }

    pub fn countdown(&self) -> Option<u32> {
        self.countdown
    }

    /// Shows why a reboot or shutdown failed, and brings the Reboot button
    /// back in place of the countdown.
    pub fn set_power_error(&mut self, error: Option<String>) {
        if error.is_some() {
            self.countdown = None;
        }
        self.power_error = error;
    }

    pub fn power_error(&self) -> Option<&str> {
        self.power_error.as_deref()
    }

    pub fn start_countdown(&mut self) {
        self.countdown.get_or_insert(REBOOT_DELAY);
    }

    /// Counts down a second; true once it is time to reboot.
    pub fn tick(&mut self) -> bool {
        match &mut self.countdown {
            Some(left) => {
                *left = left.saturating_sub(1);
                *left == 0
            }
            None => false,
        }
    }

    /// An erase leaves nothing to boot, so its primary action returns to the
    /// start page rather than rebooting.
    pub fn starts_over(&self) -> bool {
        self.erase
    }

    fn succeeded(&self) -> bool {
        self.state == RunState::Succeeded
    }

    /// The result in words; the error itself is shown beneath it.
    pub fn message(&self) -> String {
        let device = &self.device;
        match (&self.state, self.erase) {
            // Never shown: a result is only made once the run has ended.
            (RunState::Running, _) => String::new(),
            (RunState::Succeeded, false) => format!(
                "Ghaf has been successfully installed on {device}.\nPlease remove the \
                 installation media and reboot the system."
            ),
            (RunState::Succeeded, true) => format!("Disk {device} has been erased successfully."),
            (
                RunState::Failed {
                    recoverable: true, ..
                },
                _,
            ) => "Secure Boot key enrollment failed, but the disk write completed \
                 successfully. You may retry enrollment or reboot without it."
                .to_string(),
            (
                RunState::Failed {
                    disk_untouched: true,
                    ..
                },
                false,
            ) => format!(
                "Installation could not start; {device} was never written. Reboot and try \
                 again."
            ),
            (
                RunState::Failed {
                    disk_untouched: true,
                    ..
                },
                true,
            ) => {
                format!("Erasing could not start; {device} was not changed.")
            }
            (RunState::Failed { .. }, false)
                if self.failure.as_ref().is_some_and(|(phase, _)| {
                    matches!(phase, Phase::Encryption | Phase::BootEntry)
                }) =>
            {
                format!(
                    "Ghaf was written to {device}, but finishing the installation failed. \
                     Reboot and install again."
                )
            }
            (RunState::Failed { .. }, false) => format!(
                "Installation failed partway through writing {device}. The disk is not \
                 bootable and no data on it can be trusted. Reboot and start over."
            ),
            (RunState::Failed { .. }, true) => format!(
                "Erasing failed partway through. {device} may be partly erased, so no data on \
                 it can be trusted."
            ),
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        match (&self.state, self.erase) {
            (
                RunState::Failed {
                    recoverable: true, ..
                },
                _,
            ) => "Secure Boot enrollment failed",
            (RunState::Failed { .. }, false) => "Installation failed",
            (RunState::Failed { .. }, true) => "Erase failed",
            (_, false) => "Installation complete",
            (_, true) => "Disk erased",
        }
        .into()
    }

    /// A success has one way on: reboot, or back to the start after an erase.
    fn show_back(&self) -> bool {
        false
    }

    fn show_next(&self) -> bool {
        self.starts_over()
    }

    fn finish_label(&self) -> String {
        "Back to start".into()
    }

    /// Only a failure needs a choice between restarting and shutting down.
    fn footer_end(&self) -> Option<Element<'_, PageMessage>> {
        (!self.succeeded()).then(|| {
            super::power_buttons(
                PageMessage::app::<Page, _>(Message::Restart),
                PageMessage::app::<Page, _>(Message::ShutDown),
            )
        })
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let mut column = widget::column::with_capacity(3)
            .spacing(8)
            .height(Length::Fill)
            .push(widget::text::body(self.message()).size(16))
            .push_maybe(self.power_error.as_deref().map(widget::text::caption));

        // Rebooting into Ghaf is the one thing left to do, sized like the
        // start page's "Install now".
        if self.succeeded() && !self.erase {
            let action: Element<'_, PageMessage> = match self.countdown {
                Some(left) => widget::text::body(format!("Rebooting in {left}…"))
                    .size(16)
                    .line_height(cosmic::iced::widget::text::LineHeight::Absolute(36.into()))
                    .into(),
                None => widget::button::suggested("Reboot")
                    .font_size(16)
                    .line_height(22)
                    .height(36)
                    .padding([0, 14])
                    .on_press(PageMessage::app::<Page, _>(Message::Reboot))
                    .into(),
            };
            column = column
                .push(widget::space::vertical())
                .push(action.apply(widget::container).center_x(Length::Fill));
        }

        if let (RunState::Failed { .. }, Some((_, error))) = (&self.state, &self.failure) {
            column = column.push(widget::text::caption(format!("Error: {error}")));
        }

        // A short write over a half-erased disk is worse than stopping: an
        // unrecoverable failure offers no Retry. `recoverable` is reserved for
        // a Secure Boot enrollment retry -- see `RunState`.
        if matches!(
            self.state,
            RunState::Failed {
                recoverable: true,
                ..
            }
        ) {
            column = column.push(
                widget::button::standard("Retry")
                    .on_press(PageMessage::app::<Page, _>(Message::Retry)),
            );
        }

        column.into()
    }
}
