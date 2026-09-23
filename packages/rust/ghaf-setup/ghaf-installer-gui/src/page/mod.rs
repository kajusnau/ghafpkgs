// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The installer wizard's pages.

pub mod complete;
pub mod confirm;
pub mod disk;
pub mod options;
pub mod running;
pub mod welcome;

use cosmic::{Element, theme, widget};
use ghaf_setup_core::disk::BlockDevice;
use ghaf_setup_ui::{Page, PageMessage};
use indexmap::IndexMap;
use std::any::TypeId;

/// Restart and shut down as icon buttons, with the COSMIC power applet's icons.
pub fn power_buttons<'a>(restart: PageMessage, shut_down: PageMessage) -> Element<'a, PageMessage> {
    let button = |icon: &'static str, tooltip: &'static str, message: PageMessage| {
        widget::tooltip(
            widget::button::icon(widget::icon::from_name(icon).symbolic(true)).on_press(message),
            widget::text::body(tooltip),
            widget::tooltip::Position::Top,
        )
    };
    widget::row::with_capacity(2)
        .spacing(theme::spacing().space_xxs)
        .push(button("system-reboot-symbolic", "Restart", restart))
        .push(button("system-shutdown-symbolic", "Shut down", shut_down))
        .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Erase,
}

/// Builds the page set for a mode. Inapplicable pages are never inserted,
/// rather than inserted and skipped at runtime -- the wizard's page list is
/// the source of truth for what the user will be asked.
///
/// The confirm page is not part of this set: it is inserted by `main.rs`
/// once a summary of the user's choices exists to build it from.
pub fn pages(
    mode: Mode,
    devices: Vec<BlockDevice>,
    setup_mode: bool,
) -> IndexMap<TypeId, Box<dyn Page>> {
    let mut pages: IndexMap<TypeId, Box<dyn Page>> = IndexMap::new();

    pages.insert(
        TypeId::of::<welcome::Page>(),
        Box::new(welcome::Page::default()),
    );
    pages.insert(
        TypeId::of::<disk::Page>(),
        Box::new(disk::Page::new(devices)),
    );

    if mode == Mode::Install {
        pages.insert(
            TypeId::of::<options::Page>(),
            Box::new(options::Page::new(setup_mode)),
        );
    }

    let mut running = running::Page::default();
    running.set_erase(mode == Mode::Erase);
    pages.insert(TypeId::of::<running::Page>(), Box::new(running));
    pages.insert(
        TypeId::of::<complete::Page>(),
        Box::new(complete::Page::new(
            running::RunState::Running,
            mode == Mode::Erase,
            String::new(),
            None,
        )),
    );

    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_install_gets_the_options_page() {
        let pages = pages(Mode::Install, vec![], true);
        let titles: Vec<_> = pages.values().map(|p| p.title()).collect();
        assert!(
            titles
                .iter()
                .any(|t| t.contains("options") || t.contains("Options"))
        );
    }

    #[test]
    fn erasing_a_disk_has_no_options_page() {
        let pages = pages(Mode::Erase, vec![], true);
        let titles: Vec<_> = pages.values().map(|p| p.title()).collect();
        assert!(
            !titles
                .iter()
                .any(|t| t.contains("options") || t.contains("Options")),
            "encryption and Secure Boot are meaningless when only erasing"
        );
    }

    #[test]
    fn every_mode_ends_on_the_complete_page() {
        for mode in [Mode::Install, Mode::Erase] {
            let pages = pages(mode, vec![], true);
            assert_eq!(pages.keys().last(), Some(&TypeId::of::<complete::Page>()));
        }
    }
}
