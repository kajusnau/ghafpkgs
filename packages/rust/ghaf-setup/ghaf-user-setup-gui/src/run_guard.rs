// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! At most one account creation at a time.

#[derive(Default)]
pub struct RunGuard {
    in_flight: bool,
}

impl RunGuard {
    /// Whether a new run may start; true marks it as in flight.
    pub fn try_start(&mut self) -> bool {
        !std::mem::replace(&mut self.in_flight, true)
    }

    pub fn finish(&mut self) {
        self.in_flight = false;
    }
}
