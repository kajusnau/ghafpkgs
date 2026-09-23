// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The page abstraction, following cosmic-initial-setup's `Page` trait so the
//! Ghaf wizards feel like part of the same first-run sequence.

use cosmic::{Element, Task, iced::Subscription, widget};
use std::any::{Any, TypeId};

/// Messages a page emits. The concrete payload is the binary's own page
/// message enum, boxed so this crate stays free of app-specific types.
#[derive(Clone, Debug)]
pub enum PageMessage {
    /// Navigate to the page at this index.
    PageOpen(usize),
    /// Leave the wizard.
    Finish,
    /// An app-specific message, routed back to the page that produced it.
    ///
    /// Tagged with the producing page's `TypeId` because a message from
    /// `init()`, `open()` or `subscription()` can resolve after the user has
    /// navigated to a different page -- unlike a message from `view()`,
    /// which always resolves while its page is still current. Without the
    /// tag, a consumer would have to assume "whatever page is on screen
    /// now", which is wrong for anything async. Routing on `TypeId` matches
    /// the `IndexMap<TypeId, Box<dyn Page>>` the pages are stored in, so
    /// the app can dispatch with `pages.get_mut(&page)` directly.
    App {
        page: TypeId,
        payload: std::sync::Arc<dyn Any + Send + Sync>,
    },
}

impl PageMessage {
    /// Builds a tagged `App` message for page `P`, filling in its `TypeId`
    /// so page code never hand-writes `TypeId::of::<P>()` at call sites.
    pub fn app<P: Page + 'static, M: Any + Send + Sync>(payload: M) -> PageMessage {
        PageMessage::App {
            page: TypeId::of::<P>(),
            payload: std::sync::Arc::new(payload),
        }
    }
}

pub trait Page {
    fn as_any(&mut self) -> &mut dyn Any;

    fn title(&self) -> String;

    fn view(&self) -> Element<'_, PageMessage> {
        widget::text::body("").into()
    }

    fn init(&mut self) -> Task<PageMessage> {
        Task::none()
    }

    fn open(&mut self) -> Task<PageMessage> {
        Task::none()
    }

    fn subscription(&self) -> Subscription<PageMessage> {
        Subscription::none()
    }

    fn dialog(&self) -> Option<Element<'_, PageMessage>> {
        None
    }

    /// Content width. cosmic-initial-setup's default; wider pages (a device
    /// list) override it.
    fn width(&self) -> f32 {
        640.0
    }

    /// Gates Next/Finish. This is where per-page validation surfaces.
    fn completed(&self) -> bool {
        true
    }

    fn optional(&self) -> bool {
        false
    }

    fn skippable(&self) -> bool {
        false
    }

    /// False for a page whose own buttons navigate, such as a choice of action.
    fn show_next(&self) -> bool {
        true
    }

    /// False for a page there is no going back from, such as a running install.
    fn show_back(&self) -> bool {
        true
    }

    /// Replaces the default title text, e.g. with a logo.
    fn header(&self) -> Option<Element<'_, PageMessage>> {
        None
    }

    /// Text links stacked at the bottom left, beside where Skip would be.
    fn links(&self) -> Vec<(&'static str, PageMessage)> {
        Vec::new()
    }

    /// Content at the bottom right, after Back and Next, e.g. power buttons.
    fn footer_end(&self) -> Option<Element<'_, PageMessage>> {
        None
    }

    /// The primary button's label on the last page, where it emits `Finish`.
    fn finish_label(&self) -> String {
        "Finish".into()
    }

    /// The primary button's label when it is not the last page.
    fn next_label(&self) -> String {
        "Next".into()
    }

    /// Ghaf's addition to the upstream trait: the confirmation page's primary
    /// action erases a disk, so default focus starts on Back instead.
    fn destructive(&self) -> bool {
        false
    }
}
