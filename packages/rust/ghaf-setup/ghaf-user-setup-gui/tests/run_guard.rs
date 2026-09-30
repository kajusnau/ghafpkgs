// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_user_setup_gui::run_guard::RunGuard;

#[test]
fn a_second_trigger_while_a_run_is_in_flight_starts_nothing() {
    let mut guard = RunGuard::default();

    assert!(guard.try_start());
    assert!(!guard.try_start());
}

#[test]
fn a_finished_run_allows_the_next_one() {
    let mut guard = RunGuard::default();
    guard.try_start();

    guard.finish();

    assert!(guard.try_start());
}
