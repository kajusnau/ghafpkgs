// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The terminal screen: the result of the install or erase.

use crate::page::running::RunState;
use cosmic::{Element, widget};
use ghaf_setup_core::progress::Phase;
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    Retry,
    Restart,
    ShutDown,
}

pub struct Page {
    state: RunState,
    erase: bool,
    device: String,
    failure: Option<(Phase, String)>,
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
        }
    }

    /// An erase leaves nothing to boot, so its primary action returns to the
    /// start page rather than rebooting.
    pub fn starts_over(&self) -> bool {
        self.erase
    }

    /// The result in words; the error itself is shown beneath it.
    pub fn message(&self) -> String {
        let device = &self.device;
        match (&self.state, self.erase) {
            // Only a placeholder before `advance_to_complete` swaps in the
            // real result; never actually shown to a user.
            (RunState::Running, _) => String::new(),
            (RunState::Succeeded, false) => format!(
                "Ghaf was installed on {device}. Remove the installation media, then reboot \
                 to start Ghaf."
            ),
            (RunState::Succeeded, true) => format!(
                "All data on {device} has been erased. You can install Ghaf, erase another \
                 disk, or shut down."
            ),
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

    /// Only an erase has a next step; otherwise the power buttons are the way out.
    fn show_back(&self) -> bool {
        false
    }

    fn show_next(&self) -> bool {
        self.starts_over()
    }

    fn finish_label(&self) -> String {
        "Back to start".into()
    }

    fn footer_end(&self) -> Option<Element<'_, PageMessage>> {
        Some(super::power_buttons(
            PageMessage::app::<Page, _>(Message::Restart),
            PageMessage::app::<Page, _>(Message::ShutDown),
        ))
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let mut column = widget::column::with_capacity(3)
            .spacing(8)
            .push(widget::text::body(self.message()));

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
