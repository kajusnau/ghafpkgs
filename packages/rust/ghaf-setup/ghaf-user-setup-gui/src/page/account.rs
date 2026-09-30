// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The account details page: username, real name, password and an optional
//! FIDO2 security key.

use cosmic::iced::Length;
use cosmic::{Element, theme, widget};
use ghaf_setup_core::homed::{AccountRequest, validate_real_name, validate_username};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    Username(String),
    RealName(String),
    Password(String),
    Confirm(String),
    ToggleFido(bool),
    Pin(String),
    TogglePasswordVisible,
}

/// The per-field messages shown to the user, quiet until there is something
/// to say.
#[derive(Debug, Default)]
pub struct AccountErrors {
    pub username: Option<String>,
    pub real_name: Option<String>,
    pub confirm: Option<String>,
}

pub struct Page {
    username: String,
    real_name: String,
    password: String,
    confirm: String,
    pin: String,
    fido: bool,
    fido_available: bool,
    password_hidden: bool,
    /// The last name checked for availability, and whether it was taken.
    availability: Option<(String, bool)>,
}

impl Page {
    pub fn new(fido_available: bool) -> Self {
        Self {
            username: String::new(),
            real_name: String::new(),
            password: String::new(),
            confirm: String::new(),
            pin: String::new(),
            fido: false,
            fido_available,
            password_hidden: true,
            availability: None,
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// Whether a security key was found; the answer may come after typing.
    pub fn set_fido_available(&mut self, available: bool) {
        self.fido_available = available;
    }

    /// Records the availability answer for that exact name.
    pub fn set_username_taken(&mut self, name: &str, taken: bool) {
        self.availability = Some((name.to_string(), taken));
    }

    pub fn errors(&self) -> AccountErrors {
        let username = if self.username.is_empty() {
            None
        } else {
            validate_username(&self.username)
                .err()
                .map(|e| e.to_string())
                .or_else(|| {
                    (self.availability == Some((self.username.clone(), true)))
                        .then(|| "That username is taken.".to_string())
                })
        };
        let real_name = if self.real_name.is_empty() {
            None
        } else {
            validate_real_name(&self.real_name).err()
        };
        let confirm = if self.confirm.is_empty() {
            None
        } else if self.confirm != self.password {
            Some("The passwords do not match.".to_string())
        } else {
            None
        };
        AccountErrors {
            username,
            real_name,
            confirm,
        }
    }

    pub fn request(&self) -> Option<AccountRequest> {
        self.completed().then(|| AccountRequest {
            username: self.username.clone(),
            real_name: self.real_name.clone(),
            password: self.password.clone(),
            fido: self.fido && self.fido_available,
            pin: (!self.pin.is_empty()).then(|| self.pin.clone()),
        })
    }

    pub fn clear_secrets(&mut self) {
        self.password.clear();
        self.confirm.clear();
        self.pin.clear();
    }

    pub fn update(&mut self, message: Message) {
        match message {
            // Characters that can never be valid are refused outright, typed
            // or pasted; what is left to explain is the format.
            Message::Username(v) => {
                let v = v.to_ascii_lowercase();
                if v.chars().all(username_char) {
                    self.username = v;
                }
            }
            Message::RealName(v) => {
                if !v.chars().any(|c| c == ':' || c.is_control()) {
                    self.real_name = v;
                }
            }
            Message::Password(v) => self.password = v,
            Message::Confirm(v) => self.confirm = v,
            Message::ToggleFido(v) => self.fido = v,
            Message::Pin(v) => self.pin = v,
            Message::TogglePasswordVisible => self.password_hidden = !self.password_hidden,
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        "Create your account".into()
    }

    fn completed(&self) -> bool {
        validate_username(&self.username).is_ok()
            && self.availability == Some((self.username.clone(), false))
            && validate_real_name(&self.real_name).is_ok()
            && !self.password.is_empty()
            && self.password == self.confirm
    }

    fn show_back(&self) -> bool {
        false
    }

    fn next_label(&self) -> String {
        "Create account".into()
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let errors = self.errors();
        let toggle = || Some(PageMessage::app::<Page, _>(Message::TogglePasswordVisible));

        let mut list = widget::list_column()
            .add(field(
                "Username",
                None,
                errors.username,
                widget::text_input("", &self.username)
                    .on_input(|v| PageMessage::app::<Page, _>(Message::Username(v))),
            ))
            .add(field(
                "Full name",
                None,
                errors.real_name,
                widget::text_input("", &self.real_name)
                    .on_input(|v| PageMessage::app::<Page, _>(Message::RealName(v))),
            ))
            .add(field(
                "Password",
                None,
                None,
                widget::secure_input("", &self.password, toggle(), self.password_hidden)
                    .on_input(|v| PageMessage::app::<Page, _>(Message::Password(v))),
            ))
            .add(field(
                "Confirm password",
                None,
                errors.confirm,
                widget::secure_input("", &self.confirm, toggle(), self.password_hidden)
                    .on_input(|v| PageMessage::app::<Page, _>(Message::Confirm(v))),
            ));

        if self.fido_available {
            list = list.add(
                widget::settings::item::builder("Use a security key (FIDO2)")
                    .toggler(self.fido, |v| {
                        PageMessage::app::<Page, _>(Message::ToggleFido(v))
                    }),
            );

            if self.fido {
                list = list.add(field(
                    "Security key PIN",
                    Some("Leave empty if your key has no PIN."),
                    None,
                    widget::secure_input("", &self.pin, toggle(), self.password_hidden)
                        .on_input(|v| PageMessage::app::<Page, _>(Message::Pin(v))),
                ));
            }
        }

        list.into()
    }
}

/// What a username may contain at all; `validate_username` judges the rest.
fn username_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
}

/// A form row: the label, with a hint or a red error under it, centred
/// against the input, whose border turns red with the error.
fn field<'a>(
    label: &'a str,
    hint: Option<&'a str>,
    error: Option<String>,
    input: widget::TextInput<'a, PageMessage>,
) -> Element<'a, PageMessage> {
    let note: Option<Element<'a, PageMessage>> = match (&error, hint) {
        (Some(error), _) => Some(
            widget::text::caption(error.clone())
                .class(theme::Text::Custom(|theme| {
                    cosmic::iced::widget::text::Style {
                        color: Some(theme.cosmic().destructive_text_color().into()),
                        ..Default::default()
                    }
                }))
                .into(),
        ),
        (None, Some(hint)) => Some(widget::text::caption(hint).into()),
        (None, None) => None,
    };
    let input = match error {
        Some(error) => input.error(error),
        None => input,
    };

    widget::settings::item_row(vec![
        widget::column::with_capacity(2)
            .push(widget::text::body(label))
            .push_maybe(note)
            .into(),
        widget::space::horizontal().into(),
        input.width(Length::Fixed(240.0)).into(),
    ])
    .into()
}
