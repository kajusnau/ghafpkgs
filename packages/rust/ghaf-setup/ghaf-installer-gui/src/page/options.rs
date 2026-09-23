// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Encryption and Secure Boot enrollment options.

use cosmic::{Element, widget};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    SetEncrypt(bool),
    SetSecureBoot(bool),
}

#[derive(Default)]
pub struct Page {
    /// True when the firmware is in Setup Mode, i.e. Secure Boot key
    /// enrollment is possible. Set once at construction from
    /// `install::boot::in_setup_mode`.
    setup_mode: bool,
    encrypt: bool,
    secure_boot: bool,
}

impl Page {
    pub fn new(setup_mode: bool) -> Self {
        Self {
            setup_mode,
            encrypt: false,
            secure_boot: false,
        }
    }

    pub fn encrypt(&self) -> bool {
        self.encrypt
    }

    pub fn set_encrypt(&mut self, encrypt: bool) {
        self.encrypt = encrypt;
    }

    pub fn secure_boot(&self) -> bool {
        self.secure_boot
    }

    /// Updates the Setup Mode flag once the async `efi-readvar` check
    /// resolves, without discarding a toggle the user already made.
    pub fn set_setup_mode(&mut self, setup_mode: bool) {
        self.setup_mode = setup_mode;
        if !setup_mode {
            self.secure_boot = false;
        }
    }

    /// Enrollment the firmware will reject must not be offered: `true` is
    /// ignored outside Setup Mode.
    pub fn set_secure_boot(&mut self, secure_boot: bool) {
        if secure_boot && !self.setup_mode {
            return;
        }
        self.secure_boot = secure_boot;
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SetEncrypt(v) => self.set_encrypt(v),
            Message::SetSecureBoot(v) => self.set_secure_boot(v),
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        "Installation options".into()
    }

    /// Both options are optional; the page never blocks Next.
    fn completed(&self) -> bool {
        true
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let list = widget::list_column().add(
            widget::settings::item::builder("Encrypt the disk").toggler(self.encrypt, |v| {
                PageMessage::app::<Page, _>(Message::SetEncrypt(v))
            }),
        );

        let secure_boot = widget::settings::item::builder("Enroll Secure Boot keys");
        if self.setup_mode {
            list.add(secure_boot.toggler(self.secure_boot, |v| {
                PageMessage::app::<Page, _>(Message::SetSecureBoot(v))
            }))
        } else {
            // A disabled toggler looks like an enabled one, so show no control at all.
            list.add(
                secure_boot
                    .description("The firmware must be in Setup Mode to enroll keys.")
                    .control(widget::text::caption("Unavailable")),
            )
        }
        .into()
    }
}
