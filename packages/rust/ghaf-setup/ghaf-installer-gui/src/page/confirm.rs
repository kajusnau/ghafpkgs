// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The destructive confirmation: the last screen before the disk is wiped.

use cosmic::iced::{Alignment, Length};
use cosmic::{Apply, Element, theme, widget};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

pub struct Summary {
    pub action: String,
    pub wipe_only: bool,
    pub device: String,
    pub encrypt: bool,
    pub secure_boot: bool,
}

pub struct Page {
    summary: Summary,
}

impl Page {
    pub fn new(summary: Summary) -> Self {
        Self { summary }
    }

    pub fn summary_lines(&self) -> Vec<String> {
        vec![
            format!("Action: {}", self.summary.action),
            format!("Target disk: {}", self.summary.device),
            format!(
                "Encryption: {}",
                if self.summary.encrypt {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
            format!(
                "Secure Boot enrollment: {}",
                if self.summary.secure_boot {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        ]
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        if self.summary.wipe_only {
            "Confirm erase"
        } else {
            "Confirm installation"
        }
        .into()
    }

    /// The primary action erases a disk; `frame()` uses this to style the
    /// button accordingly.
    fn destructive(&self) -> bool {
        true
    }

    fn next_label(&self) -> String {
        if self.summary.wipe_only {
            "Erase"
        } else {
            "Install"
        }
        .into()
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let spacing = theme::spacing();
        let on_off = |on: bool| if on { "Enabled" } else { "Disabled" };

        let mut rows = vec![
            ("Action", self.summary.action.as_str()),
            ("Target disk", self.summary.device.as_str()),
        ];
        if !self.summary.wipe_only {
            rows.push(("Encryption", on_off(self.summary.encrypt)));
            rows.push(("Secure Boot enrollment", on_off(self.summary.secure_boot)));
        }

        let table = rows
            .into_iter()
            .fold(widget::list_column(), |list, (label, value)| {
                list.add(widget::settings::item(label, widget::text::heading(value)))
            });

        let warning = widget::row::with_capacity(2)
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name("dialog-warning-symbolic").size(20))
            .push(widget::text::body(format!(
                "All data on {} will be permanently erased.",
                self.summary.device
            )))
            .apply(widget::container)
            .padding(spacing.space_s)
            .width(Length::Fill)
            .class(theme::Container::custom(|theme| {
                let mut style = widget::warning_container(theme);
                style.border.radius = theme.cosmic().corner_radii.radius_s.into();
                style
            }));

        widget::column::with_capacity(2)
            .spacing(spacing.space_m)
            .push(table)
            .push(warning)
            .into()
    }
}
