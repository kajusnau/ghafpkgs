// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The install as a phase checklist, which stays up with the result
//! beneath it once the run ends.

use super::complete;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::{Apply, Element, theme, widget};
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

/// The log is a debugging aid; the journal has the full record.
pub const MAX_LOG_LINES: usize = 500;

#[derive(Clone, Debug)]
pub enum Message {
    /// Shows or hides the detailed log.
    ToggleLog,
}

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
    show_log: bool,
    last_phase: Option<Phase>,
    failure: Option<(Phase, String)>,
    erase: bool,
    /// How far the image write has got, from bmaptool.
    fraction: Option<f32>,
    /// Set once the run has ended.
    result: Option<complete::Page>,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            state: RunState::Running,
            plan: Vec::new(),
            current: None,
            done: Vec::new(),
            log: Vec::new(),
            show_log: false,
            last_phase: None,
            failure: None,
            erase: false,
            fraction: None,
            result: None,
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

    /// The image write's progress while it runs, for its row.
    pub fn fraction(&self) -> Option<f32> {
        self.fraction
    }

    /// Shows the outcome under the checklist.
    pub fn set_result(&mut self, result: complete::Page) {
        self.result = Some(result);
    }

    pub fn result(&self) -> Option<&complete::Page> {
        self.result.as_ref()
    }

    pub fn result_mut(&mut self) -> Option<&mut complete::Page> {
        self.result.as_mut()
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    /// The last phase touched, for synthesising a `Failed` event when the
    /// orchestrator returns an error without one having already been sent.
    pub fn last_phase(&self) -> Option<Phase> {
        self.last_phase
    }

    /// The phase that failed and why, for the result.
    pub fn failure(&self) -> Option<(Phase, String)> {
        self.failure.clone()
    }

    /// Resets to a fresh running state, for a Retry that must re-show
    /// progress rather than sit on the previous run's terminal screen.
    pub fn reset(&mut self) {
        let show_log = self.show_log;
        *self = Self::default();
        self.show_log = show_log;
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ToggleLog => self.show_log = !self.show_log,
        }
    }

    pub fn log_shown(&self) -> bool {
        self.show_log
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
            ProgressEvent::PhaseProgress { phase, fraction } => {
                self.last_phase = Some(phase);
                if phase == Phase::WriteImage {
                    self.fraction = Some(fraction);
                }
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
        if let Some(result) = &self.result {
            return result.title();
        }
        if self.erase {
            "Erasing disk"
        } else {
            "Installing Ghaf"
        }
        .into()
    }

    /// There is no going back from a write, running or finished.
    fn show_back(&self) -> bool {
        false
    }

    /// Nothing to press while running; afterwards, as the result says.
    fn show_next(&self) -> bool {
        self.result.as_ref().is_some_and(|r| r.show_next())
    }

    fn finish_label(&self) -> String {
        self.result
            .as_ref()
            .map_or_else(|| "Finish".into(), |r| r.finish_label())
    }

    fn footer_end(&self) -> Option<Element<'_, PageMessage>> {
        self.result.as_ref().and_then(|r| r.footer_end())
    }

    fn completed(&self) -> bool {
        !matches!(self.state, RunState::Running)
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let spacing = theme::spacing();
        let mut column =
            widget::column::with_capacity(self.plan.len() + 3).spacing(spacing.space_s);

        for phase in &self.plan {
            let step = self.step(*phase);
            // When only erasing, wiping is the whole job, not preparation.
            let label = match (self.erase && *phase == Phase::Wipe, step) {
                (true, Step::Pending) => "Erase all data",
                (true, _) => "Erasing all data",
                (false, Step::Pending) => phase.label(),
                (false, _) => phase.active_label(),
            };
            let (indicator, text): (Element<'_, PageMessage>, Element<'_, PageMessage>) = match step
            {
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
            let percent = (step == Step::Running && *phase == Phase::WriteImage)
                .then_some(self.fraction)
                .flatten()
                .map(|f| widget::text::body(format!("{:.0}%", f * 100.0)));
            column = column.push(
                widget::row::with_capacity(3)
                    .spacing(spacing.space_s)
                    .align_y(Alignment::Center)
                    .push(indicator)
                    .push(text)
                    .push_maybe(percent),
            );
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
            // Anchored to the bottom so it follows new lines as they come.
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
        }

        // The log and the result share what room is left.
        if self.show_log || self.result.is_some() {
            column = column.height(Length::Fill);
        }

        if let Some(result) = &self.result {
            column = column.push(result.view());
        }
        column.into()
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
