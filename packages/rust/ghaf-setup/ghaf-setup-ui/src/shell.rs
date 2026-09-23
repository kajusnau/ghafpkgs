// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Window sizing, navigation, and the centred frame every page renders into.

use crate::page::{Page, PageMessage};
use cosmic::iced::{Alignment, Length};
use cosmic::{Apply, Element, cosmic_theme, theme, widget};

/// cosmic-initial-setup's window limits, applied to the card itself: kiosk
/// compositors such as cage make every window fullscreen.
pub const MAX_WIDTH: f32 = 900.0;
pub const MAX_HEIGHT: f32 = 650.0;

/// The button row's width, fixed so Back and Next stay put between pages.
const BUTTON_ROW_WIDTH: f32 = 640.0;
const CARD_HEIGHT: f32 = 560.0;

/// Identifies the Back button so a destructive page's caller can steer
/// default focus away from the primary action (spec requirement: a stray
/// Enter must not fire a destructive action).
pub fn back_button_id() -> widget::Id {
    widget::Id::new("ghaf-setup-wizard-back-button")
}

/// Navigation over a fixed page list. Pages never choose their successor;
/// the page set is decided up front by the binary's `pages(mode)`.
#[derive(Debug, Clone)]
pub struct Nav {
    current: usize,
    count: usize,
}

impl Nav {
    pub fn new(count: usize) -> Self {
        Self { current: 0, count }
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn go_to(&mut self, index: usize) {
        if index < self.count {
            self.current = index;
        }
    }

    pub fn back_target(&self) -> Option<usize> {
        self.current.checked_sub(1)
    }

    pub fn next_target(&self) -> Option<usize> {
        let next = self.current + 1;
        (next < self.count).then_some(next)
    }

    pub fn is_last(&self) -> bool {
        self.current + 1 >= self.count
    }
}

/// Renders title, page content and the button row into a centred column.
pub fn frame<'a>(page: &'a dyn Page, nav: &Nav) -> Element<'a, PageMessage> {
    let cosmic_theme::Spacing {
        space_xxs,
        space_m,
        space_l,
        space_xl,
        ..
    } = theme::spacing();

    // An `optional` page can be skipped forward to the next page; a
    // `skippable` page (no next page makes sense, e.g. it is itself
    // optional-entry-point) closes the whole wizard instead. These are
    // different actions with different labels -- see cosmic-initial-setup's
    // `App::view`.
    let skip_button = page
        .optional()
        .then(|| widget::button::link("Skip").on_press(PageMessage::PageOpen(nav.current() + 1)))
        .or_else(|| {
            page.skippable()
                .then(|| widget::button::link("Skip setup").on_press(PageMessage::Finish))
        });

    let links = page.links();
    let links = (!links.is_empty()).then(|| {
        widget::column::with_children(
            links
                .into_iter()
                .map(|(label, message)| link(label, message)),
        )
        .spacing(space_xxs)
    });

    let mut buttons = widget::row::with_capacity(4)
        .spacing(space_xxs)
        .align_y(Alignment::End)
        .push_maybe(skip_button)
        .push_maybe(links)
        .push(widget::space::horizontal());

    if let Some(back) = nav.back_target().filter(|_| page.show_back()) {
        buttons = buttons.push(
            widget::button::standard("Back")
                .id(back_button_id())
                .on_press(PageMessage::PageOpen(back)),
        );
    }

    if page.show_next() {
        let label = if nav.is_last() {
            page.finish_label()
        } else {
            page.next_label()
        };
        let action = if nav.is_last() {
            PageMessage::Finish
        } else {
            PageMessage::PageOpen(nav.current() + 1)
        };

        let mut primary = widget::button::suggested(label);
        if page.completed() {
            primary = primary.on_press(action);
        }
        buttons = buttons.push(primary);
    }
    buttons = buttons.push_maybe(page.footer_end());

    let content = widget::column::with_capacity(5)
        .push(page.header().unwrap_or_else(|| {
            widget::text::title2(page.title())
                .center()
                .width(Length::Fill)
                .into()
        }))
        .push(widget::space::vertical().height(space_l))
        .push(
            page.view()
                .apply(widget::container)
                .width(page.width())
                .height(Length::Fill),
        )
        .push(widget::space::vertical().height(space_m))
        .push(buttons.width(BUTTON_ROW_WIDTH))
        .align_x(Alignment::Center);

    // Hugs the content column; a fixed height keeps Back and Next in place.
    let card = content
        .width(BUTTON_ROW_WIDTH)
        .apply(widget::container)
        .padding([space_xl, space_l, space_l, space_l])
        .height(CARD_HEIGHT)
        .class(theme::Container::custom(card_style));

    card.apply(widget::container)
        .center(Length::Fill)
        .class(theme::Container::Background)
        .into()
}

/// A text link: iced's rich text underlines it and shows a hand while hovered.
fn link<'a>(label: &'static str, message: PageMessage) -> Element<'a, PageMessage> {
    let color = theme::active().cosmic().accent_text_color();
    cosmic::iced::widget::rich_text([cosmic::iced::widget::span(label).link(message).color(color)])
        .size(14)
        .on_link_click(|message| message)
        .into()
}

fn card_style(theme: &cosmic::Theme) -> widget::container::Style {
    let cosmic = theme.cosmic();
    let primary = cosmic.primary(false);
    widget::container::Style {
        icon_color: Some(primary.on.into()),
        text_color: Some(primary.on.into()),
        background: Some(cosmic::iced::Background::Color(primary.base.into())),
        border: cosmic::iced::Border {
            radius: cosmic.corner_radii.radius_m.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
