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
}

#[derive(Default)]
pub struct Page {
    devices: Vec<BlockDevice>,
    selected: Option<usize>,
    /// Set when the installer's own boot medium could not be identified, so
    /// the exclusion in `disk::enumerate` may have silently done nothing.
    boot_device_warning: Option<String>,
}

impl Page {
    pub fn new(devices: Vec<BlockDevice>) -> Self {
        Self::new_with_selection(devices, None, None)
    }

    /// Like `new`, but re-selects the device at `selected_path` if it is
    /// still present in `devices` -- a late device-list refresh must not
    /// silently drop the user's choice -- and carries a warning when the
    /// installer's own boot medium could not be excluded. Otherwise the first
    /// disk is pre-selected; removable drives sort last so that is never the
    /// installation media.
    pub fn new_with_selection(
        mut devices: Vec<BlockDevice>,
        selected_path: Option<&str>,
        boot_device_warning: Option<String>,
    ) -> Self {
        devices.sort_by_key(|d| d.removable);
        let selected = match selected_path {
            Some(path) => devices.iter().position(|d| d.path == path),
            None => (!devices.is_empty()).then_some(0),
        };
        Self {
            devices,
            selected,
            boot_device_warning,
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

    pub fn selected_is_removable(&self) -> bool {
        self.selected().is_some_and(|d| d.removable)
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Select(i) => self.select(i),
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
        if self.devices.is_empty() {
            return widget::text::body(
                "No disks found. The disk the installer booted from is not listed; \
                 connect another disk and restart the installer.",
            )
            .into();
        }

        let mut list = widget::list_column();

        if let Some(warning) = &self.boot_device_warning {
            list = list.add(widget::text::caption(warning.clone()));
        }

        for (i, device) in self.devices.iter().enumerate() {
            let name = device
                .model
                .clone()
                .unwrap_or_else(|| "Unknown disk".into());
            // Inline rather than a post-selection confirm dialog: the warning
            // has to be visible while choosing, not after.
            let detail = if device.removable {
                format!(
                    "{} · Removable drive, may be the installation media",
                    device.path
                )
            } else {
                device.path.clone()
            };

            let label = widget::row::with_capacity(3)
                .align_y(Alignment::Center)
                .push(
                    widget::column::with_capacity(2)
                        .spacing(theme::spacing().space_xxxs)
                        .push(widget::text::body(name))
                        .push(widget::text::caption(detail)),
                )
                .push(widget::space::horizontal())
                .push(widget::text::heading(device.human_size()))
                .width(Length::Fill);

            list = list.add(
                widget::radio(label, i, self.selected, |i| {
                    PageMessage::app::<Page, _>(Message::Select(i))
                })
                .width(Length::Fill),
            );
        }

        widget::scrollable(list).height(Length::Fill).into()
    }
}
