// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_ui::shell::Nav;

#[test]
fn starts_on_the_first_page_with_no_way_back() {
    let nav = Nav::new(3);
    assert_eq!(nav.current(), 0);
    assert_eq!(nav.back_target(), None);
    assert_eq!(nav.next_target(), Some(1));
    assert!(!nav.is_last());
}

#[test]
fn the_last_page_finishes_instead_of_advancing() {
    let mut nav = Nav::new(3);
    nav.go_to(2);
    assert_eq!(nav.next_target(), None);
    assert!(nav.is_last());
    assert_eq!(nav.back_target(), Some(1));
}

#[test]
fn go_to_ignores_an_out_of_range_index() {
    let mut nav = Nav::new(3);
    nav.go_to(99);
    assert_eq!(
        nav.current(),
        0,
        "an invalid target must not move the wizard"
    );
}

#[test]
fn a_single_page_wizard_only_finishes() {
    let nav = Nav::new(1);
    assert_eq!(nav.back_target(), None);
    assert_eq!(nav.next_target(), None);
    assert!(nav.is_last());
}
