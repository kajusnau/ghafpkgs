// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The start page: one "Install now" button, with erasing a disk and leaving
//! for a shell as links at the bottom left.

use cosmic::iced::{Alignment, Length};
use cosmic::{Apply, Element, theme, widget};
use ghaf_setup_ui::{Page as PageTrait, PageMessage};
use std::any::Any;

/// A copy of ghaf-artwork's ghaf-logo-512px.png.
const LOGO: &[u8] = include_bytes!("../../assets/ghaf-logo.png");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    Erase,
}

#[derive(Clone, Debug)]
pub enum Message {
    Choose(Action),
    ExitToShell,
    Restart,
    ShutDown,
}

#[derive(Default)]
pub struct Page {
    action: Option<Action>,
}

impl Page {
    pub fn choose(&mut self, action: Action) {
        self.action = Some(action);
    }

    pub fn action(&self) -> Option<Action> {
        self.action
    }

    pub fn update(&mut self, message: Message) {
        if let Message::Choose(action) = message {
            self.choose(action);
        }
    }
}

fn message(message: Message) -> PageMessage {
    PageMessage::app::<Page, _>(message)
}

impl PageTrait for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        "Ghaf".into()
    }

    fn completed(&self) -> bool {
        self.action.is_some()
    }

    fn show_next(&self) -> bool {
        false
    }

    fn header(&self) -> Option<Element<'_, PageMessage>> {
        let logo = widget::icon(widget::icon::from_raster_bytes(LOGO)).size(40);
        let header = widget::row::with_capacity(2)
            .spacing(theme::spacing().space_s)
            .align_y(Alignment::Center)
            .push(logo)
            .push(widget::text::title2(self.title()))
            .apply(widget::container)
            .center_x(Length::Fill);
        Some(header.into())
    }

    fn links(&self) -> Vec<(&'static str, PageMessage)> {
        vec![
            ("Erase a disk", message(Message::Choose(Action::Erase))),
            ("Exit to shell", message(Message::ExitToShell)),
        ]
    }

    fn footer_end(&self) -> Option<Element<'_, PageMessage>> {
        Some(super::power_buttons(
            message(Message::Restart),
            message(Message::ShutDown),
        ))
    }

    fn view(&self) -> Element<'_, PageMessage> {
        // A little larger than a stock text button.
        widget::button::suggested("Install now")
            .font_size(16)
            .line_height(22)
            .height(36)
            .padding([0, 14])
            .on_press(message(Message::Choose(Action::Install)))
            .apply(widget::container)
            .center(Length::Fill)
            .into()
    }
}
