// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Choose the disk to install to.

use cosmic::iced::{Alignment, Length};
use cosmic::{Element, theme, widget};
use ghaf_setup_core::disk::BlockDevice;
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    Select(usize),
    ToggleUnavailable,
}

#[derive(Default)]
pub struct Page {
    /// The disks that can be chosen; `selected` indexes only these, so an
    /// unavailable disk can never be selected.
    devices: Vec<BlockDevice>,
    /// Read-only disks and the installation media, listed for reference.
    unavailable: Vec<BlockDevice>,
    show_unavailable: bool,
    selected: Option<usize>,
    /// Why the list may be incomplete or restricted, e.g. the disks could
    /// not be read or the installer's own disk could not be identified.
    warning: Option<String>,
}

impl Page {
    pub fn new(devices: Vec<BlockDevice>) -> Self {
        Self::new_with_selection(devices, None, None)
    }

    /// Like `new`, but re-selects the device at `selected_path` if it is
    /// still present in `devices` -- a late device-list refresh must not
    /// silently drop the user's choice -- and carries a warning to show above
    /// the list. Otherwise the
    /// first disk is pre-selected; removable drives sort last so that is a
    /// fixed disk where there is one.
    pub fn new_with_selection(
        devices: Vec<BlockDevice>,
        selected_path: Option<&str>,
        warning: Option<String>,
    ) -> Self {
        let (mut devices, unavailable): (Vec<_>, Vec<_>) =
            devices.into_iter().partition(BlockDevice::available);
        devices.sort_by_key(|d| d.removable);
        let selected = match selected_path {
            Some(path) => devices.iter().position(|d| d.path == path),
            None => (!devices.is_empty()).then_some(0),
        };
        Self {
            devices,
            unavailable,
            show_unavailable: false,
            selected,
            warning,
        }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.devices.len() {
            self.selected = Some(index);
        }
    }

    pub fn selected(&self) -> Option<&BlockDevice> {
        self.selected.and_then(|i| self.devices.get(i))
    }

    pub fn unavailable(&self) -> &[BlockDevice] {
        &self.unavailable
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Select(i) => self.select(i),
            Message::ToggleUnavailable => self.show_unavailable = !self.show_unavailable,
        }
    }
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        "Select target disk".into()
    }

    fn completed(&self) -> bool {
        self.selected.is_some()
    }

    fn view(&self) -> Element<'_, PageMessage> {
        let spacing = theme::spacing();
        let mut list = widget::list_column();

        if let Some(warning) = &self.warning {
            list = list.add(widget::text::caption(warning.clone()));
        }

        if self.devices.is_empty() {
            list = list.add(widget::text::body(
                "No disks available to install to. Connect a disk and restart the installer.",
            ));
        }

        for (i, device) in self.devices.iter().enumerate() {
            let detail = device.removable.then(|| {
                widget::row::with_capacity(2)
                    .spacing(spacing.space_xxxs)
                    .align_y(Alignment::Center)
                    .push(
                        widget::icon::from_name("dialog-warning-symbolic")
                            .size(12)
                            .icon()
                            .class(theme::Svg::custom(|theme| widget::svg::Style {
                                color: Some(theme.cosmic().warning_color().into()),
                            })),
                    )
                    .push(widget::text::caption("Removable drive"))
            });
            list = list.add(
                widget::radio(row(device, detail), i, self.selected, |i| {
                    PageMessage::app::<Page, _>(Message::Select(i))
                })
                .width(Length::Fill),
            );
        }

        let mut column = widget::column::with_capacity(3)
            .spacing(spacing.space_s)
            .push(list);

        if !self.unavailable.is_empty() {
            column = column.push(
                widget::button::text(format!("Unavailable disks ({})", self.unavailable.len()))
                    .trailing_icon(widget::icon::from_name(if self.show_unavailable {
                        "go-up-symbolic"
                    } else {
                        "go-down-symbolic"
                    }))
                    .padding([spacing.space_xxxs, 0])
                    .on_press(PageMessage::app::<Page, _>(Message::ToggleUnavailable)),
            );
        }

        if self.show_unavailable {
            let mut unavailable = widget::list_column();
            for device in &self.unavailable {
                let reason = if device.installation_media {
                    "Installation media"
                } else if device.read_only {
                    "READ-ONLY"
                } else {
                    "May be the installation media"
                };
                unavailable = unavailable.add(
                    widget::container(row(device, Some(widget::text::caption(reason))))
                        .class(theme::Container::custom(dimmed)),
                );
            }
            column = column.push(unavailable);
        }

        widget::scrollable(column).height(Length::Fill).into()
    }
}

/// A disk's name, path and size, with an optional note after the path.
fn row<'a>(
    device: &'a BlockDevice,
    note: Option<impl Into<Element<'a, PageMessage>>>,
) -> Element<'a, PageMessage> {
    let spacing = theme::spacing();
    let name = device.model.as_deref().unwrap_or("Unknown disk");
    let detail = widget::row::with_capacity(3)
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center)
        .push(widget::text::caption(device.path.as_str()))
        .push_maybe(note.map(|note| {
            widget::row::with_capacity(2)
                .spacing(spacing.space_xxs)
                .align_y(Alignment::Center)
                .push(widget::text::caption("·"))
                .push(note)
        }));

    widget::row::with_capacity(3)
        .align_y(Alignment::Center)
        .push(
            widget::column::with_capacity(2)
                .spacing(spacing.space_xxxs)
                .push(widget::text::body(name))
                .push(detail),
        )
        .push(widget::space::horizontal())
        .push(widget::text::heading(device.human_size()))
        .width(Length::Fill)
        .into()
}

/// Unavailable disks: their text at half strength.
fn dimmed(theme: &cosmic::Theme) -> widget::container::Style {
    let mut color: cosmic::iced::Color = theme.cosmic().on_bg_color().into();
    color.a = 0.5;
    widget::container::Style {
        text_color: Some(color),
        ..Default::default()
    }
}
