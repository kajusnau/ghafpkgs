// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The account creation as it runs, then its result: a plain success, a
//! recovery key to save, or a failure that offers Back to the form.

use cosmic::iced::Length;
use cosmic::iced::widget::qr_code;
use cosmic::{Apply, Element, theme, widget};
use ghaf_setup_core::homed::RecoveryKey;
use ghaf_setup_core::progress::ProgressEvent;
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

/// The log is a debugging aid; the journal has the full record.
pub const MAX_LOG_LINES: usize = 500;

#[derive(Clone, Debug)]
pub enum Message {
    /// Shows or hides the detailed log.
    ToggleLog,
    /// The user has ticked "I have saved my recovery key".
    Acknowledge(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Created { key: Option<RecoveryKey> },
    Failed(String),
}

pub struct Page {
    log: Vec<String>,
    show_log: bool,
    failure: Option<String>,
    outcome: Outcome,
    acknowledged: bool,
    fido: bool,
    qr: Option<qr_code::Data>,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            log: Vec::new(),
            show_log: false,
            failure: None,
            outcome: Outcome::Running,
            acknowledged: false,
            fido: false,
            qr: None,
        }
    }
}

impl Page {
    /// Whether to show the security key touch hint while running.
    pub fn set_fido(&mut self, fido: bool) {
        self.fido = fido;
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome.clone()
    }

    pub fn apply(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Log(line) => {
                self.log.push(line);
                if self.log.len() > MAX_LOG_LINES {
                    self.log.remove(0);
                }
            }
            ProgressEvent::Failed { message, .. } => {
                self.failure = Some(message.clone());
                self.log.push(message);
            }
            ProgressEvent::PhaseStarted(_)
            | ProgressEvent::PhaseProgress { .. }
            | ProgressEvent::PhaseFinished(_) => {}
        }
    }

    /// Ends the run with its result. Comes after every progress event (see
    /// `pace::run`), so a `Failed` already reported is kept as it is.
    pub fn conclude(&mut self, result: Result<Option<RecoveryKey>, String>) {
        self.outcome = match result {
            Ok(key) => {
                self.qr = key.as_ref().and_then(|key| qr_code::Data::new(&key.0).ok());
                Outcome::Created { key }
            }
            Err(e) => Outcome::Failed(self.failure.clone().unwrap_or(e)),
        };
    }

    /// Resets to a fresh running state, for a Retry that must re-show
    /// progress rather than sit on the previous run's terminal screen.
    pub fn reset(&mut self) {
        let show_log = self.show_log;
        *self = Self {
            show_log,
            ..Self::default()
        };
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ToggleLog => self.show_log = !self.show_log,
            Message::Acknowledge(v) => self.acknowledged = v,
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        match &self.outcome {
            Outcome::Running => "Creating your account",
            Outcome::Created { .. } => "Account created",
            Outcome::Failed(_) => "Account creation failed",
        }
        .into()
    }

    /// Back to the form after a failure, where nothing was created.
    fn show_back(&self) -> bool {
        matches!(self.outcome, Outcome::Failed(_))
    }

    fn show_next(&self) -> bool {
        matches!(self.outcome, Outcome::Created { .. })
    }

    fn finish_label(&self) -> String {
        "Finish".into()
    }

    fn completed(&self) -> bool {
        match &self.outcome {
            Outcome::Created { key: None } => true,
            Outcome::Created { key: Some(_) } => self.acknowledged,
            Outcome::Running | Outcome::Failed(_) => false,
        }
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let spacing = theme::spacing();
        let mut column = widget::column::with_capacity(4).spacing(spacing.space_s);

        match &self.outcome {
            Outcome::Running => {
                column = column.push(
                    widget::row::with_capacity(2)
                        .spacing(spacing.space_s)
                        .push(widget::progress_bar::indeterminate_circular().size(20.0))
                        .push(widget::text::body("Creating your account…")),
                );
                if self.fido {
                    column = column.push(widget::text::caption(
                        "Touch your security key when it blinks.",
                    ));
                }
            }
            Outcome::Created { key: Some(key) } => {
                column = column
                    .push(widget::text::body(
                        "Save this recovery key somewhere safe. It unlocks your files if you \
                         forget your password, and it will not be shown again.",
                    ))
                    .push(
                        widget::text::monotext(key.0.as_str())
                            .size(16)
                            .apply(widget::container)
                            .padding(spacing.space_s)
                            .class(theme::Container::Secondary),
                    );
                if let Some(qr) = &self.qr {
                    column = column.push(widget::qr_code(qr));
                }
                column = column.push(
                    widget::checkbox(self.acknowledged)
                        .label("I have saved my recovery key")
                        .on_toggle(|v| PageMessage::app::<Page, _>(Message::Acknowledge(v))),
                );
            }
            Outcome::Created { key: None } => {
                column = column.push(widget::text::body("Your account is ready."));
            }
            Outcome::Failed(message) => {
                column = column.push(widget::text::body(message.as_str()));
            }
        }

        let toggle = widget::button::text(if self.show_log {
            "Hide details"
        } else {
            "Show details"
        })
        .trailing_icon(widget::icon::from_name(if self.show_log {
            "go-up-symbolic"
        } else {
            "go-down-symbolic"
        }))
        .padding([spacing.space_xxxs, 0])
        .on_press(PageMessage::app::<Page, _>(Message::ToggleLog));
        column = column.push(toggle);

        if self.show_log {
            let lines = self
                .log
                .iter()
                .map(|line| widget::text::monotext(line.as_str()).size(12).into());
            column = column.push(
                widget::column::with_children(lines)
                    .padding(spacing.space_xxs)
                    .apply(widget::scrollable)
                    .anchor_bottom()
                    .width(Length::Fill)
                    .apply(widget::container)
                    .height(Length::Fill)
                    .class(theme::Container::Secondary),
            );
            column = column.height(Length::Fill);
        }

        column.into()
    }
}
