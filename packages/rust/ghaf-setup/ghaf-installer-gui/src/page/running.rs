// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The install in progress: a phase checklist over a bounded log.

use cosmic::iced::{Alignment, Color};
use cosmic::{Element, theme, widget};
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

/// The log is a debugging aid on a screen nobody scrolls back through; the
/// journal has the full record.
pub const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Running,
    Succeeded,
    /// `recoverable` means Secure Boot enrollment alone can be retried;
    /// `disk_untouched` means no phase ever started, so it is safe to say
    /// nothing was written even though `recoverable` is false.
    Failed {
        recoverable: bool,
        disk_untouched: bool,
    },
}

/// Where a phase stands, for its row on the checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Pending,
    Running,
    Done,
}

pub struct Page {
    state: RunState,
    /// The phases this run will go through, in order.
    plan: Vec<Phase>,
    current: Option<Phase>,
    done: Vec<Phase>,
    log: Vec<String>,
    last_phase: Option<Phase>,
    failure: Option<(Phase, String)>,
    erase: bool,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            state: RunState::Running,
            plan: Vec::new(),
            current: None,
            done: Vec::new(),
            log: Vec::new(),
            last_phase: None,
            failure: None,
            erase: false,
        }
    }
}

impl Page {
    pub fn set_erase(&mut self, erase: bool) {
        self.erase = erase;
    }

    /// The phases to list, from `InstallRequest::phases`.
    pub fn set_plan(&mut self, plan: Vec<Phase>) {
        self.plan = plan;
    }

    pub fn plan(&self) -> &[Phase] {
        &self.plan
    }

    pub fn step(&self, phase: Phase) -> Step {
        if self.done.contains(&phase) {
            Step::Done
        } else if self.current == Some(phase) {
            Step::Running
        } else {
            Step::Pending
        }
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    /// The last phase touched, for synthesising a `Failed` event when the
    /// orchestrator returns an error without one having already been sent.
    pub fn last_phase(&self) -> Option<Phase> {
        self.last_phase
    }

    /// The phase that failed and why, for the result screen.
    pub fn failure(&self) -> Option<(Phase, String)> {
        self.failure.clone()
    }

    /// Resets to a fresh running state, for a Retry that must re-show
    /// progress rather than sit on the previous run's terminal screen.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Called when the orchestrator returns Ok.
    pub fn finish(&mut self) {
        if matches!(self.state, RunState::Running) {
            self.state = RunState::Succeeded;
        }
    }

    pub fn apply(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::PhaseStarted(phase) => {
                self.last_phase = Some(phase);
                self.current = Some(phase);
            }
            ProgressEvent::PhaseProgress { phase, .. } => {
                self.last_phase = Some(phase);
            }
            ProgressEvent::PhaseFinished(phase) => {
                self.last_phase = Some(phase);
                if !self.done.contains(&phase) {
                    self.done.push(phase);
                }
                if self.current == Some(phase) {
                    self.current = None;
                }
            }
            ProgressEvent::Log(line) => {
                self.log.push(line);
                if self.log.len() > MAX_LOG_LINES {
                    self.log.remove(0);
                }
            }
            ProgressEvent::Failed {
                phase,
                message,
                recoverable,
            } => {
                let disk_untouched = self.last_phase.is_none();
                self.failure = Some((phase, message.clone()));
                self.log.push(message);
                self.state = RunState::Failed {
                    recoverable,
                    disk_untouched,
                };
            }
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        if self.erase {
            "Erasing disk"
        } else {
            "Installing Ghaf"
        }
        .into()
    }

    /// Blocked while running so the wizard cannot be advanced past a live
    /// write; released once there is a result to show.
    fn show_back(&self) -> bool {
        false
    }

    /// The result page opens by itself when the run ends.
    fn show_next(&self) -> bool {
        false
    }

    fn completed(&self) -> bool {
        !matches!(self.state, RunState::Running)
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let spacing = theme::spacing();
        let mut column =
            widget::column::with_capacity(self.plan.len() + 1).spacing(spacing.space_s);

        for phase in &self.plan {
            // When only erasing, wiping is the whole job, not preparation.
            let label = if self.erase && *phase == Phase::Wipe {
                "Erasing all data"
            } else {
                phase.label()
            };
            let (indicator, text): (Element<'_, PageMessage>, Element<'_, PageMessage>) =
                match self.step(*phase) {
                    Step::Pending => (
                        widget::space::horizontal().width(20).into(),
                        widget::text::body(label)
                            .class(theme::Text::Custom(dimmed))
                            .into(),
                    ),
                    Step::Running => (
                        widget::progress_bar::indeterminate_circular()
                            .size(20.0)
                            .into(),
                        widget::text::heading(label).into(),
                    ),
                    Step::Done => (
                        widget::icon::from_name("object-select-symbolic")
                            .size(20)
                            .icon()
                            .class(theme::Svg::custom(|theme| widget::svg::Style {
                                color: Some(theme.cosmic().accent_color().into()),
                            }))
                            .into(),
                        widget::text::body(label).into(),
                    ),
                };
            column = column.push(
                widget::row::with_capacity(2)
                    .spacing(spacing.space_s)
                    .align_y(Alignment::Center)
                    .push(indicator)
                    .push(text),
            );
        }

        column
            .push(widget::text::caption(
                self.log.last().cloned().unwrap_or_default(),
            ))
            .into()
    }
}

/// Pending steps: the body text colour at half strength.
fn dimmed(theme: &cosmic::Theme) -> cosmic::iced::widget::text::Style {
    let mut color: Color = theme.cosmic().on_bg_color().into();
    color.a = 0.5;
    cosmic::iced::widget::text::Style {
        color: Some(color),
        ..Default::default()
    }
}
