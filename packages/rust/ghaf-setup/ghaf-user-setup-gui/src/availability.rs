// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Whether a username is free, checked while the user types.

use ghaf_setup_core::homed::{username_available, validate_username};
use ghaf_setup_core::proc::CommandRunner;
use std::time::Duration;

/// Long enough not to run getent on every keystroke.
pub fn debounce() -> Duration {
    Duration::from_millis(400)
}

/// `Some(taken)`, or `None` when the name is invalid or getent failed:
/// an unknown answer must never unlock Next.
pub async fn check(runner: &dyn CommandRunner, name: &str) -> Option<bool> {
    validate_username(name).ok()?;
    username_available(runner, name)
        .await
        .ok()
        .map(|free| !free)
}
