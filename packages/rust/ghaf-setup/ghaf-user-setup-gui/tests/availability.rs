// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::proc::RecordingRunner;
use ghaf_user_setup_gui::availability::check;

#[tokio::test]
async fn an_invalid_name_is_not_looked_up() {
    let runner = RecordingRunner::new();
    assert_eq!(check(&runner, "Bad Name").await, None);
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn a_lookup_failure_is_unknown_not_free() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "nsswitch broken");
    assert_eq!(check(&runner, "alice").await, None);
}

#[tokio::test]
async fn a_free_name_is_reported_free() {
    let runner = RecordingRunner::new();
    runner.push_fail(2, "");
    assert_eq!(check(&runner, "alice").await, Some(false));
}
