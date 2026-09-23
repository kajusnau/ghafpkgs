// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! ghaf-setup-core must stay testable without a compositor. If this fails,
//! a UI crate has leaked into the logic crate -- move the code, do not
//! relax the test.

#[test]
fn core_has_no_windowing_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in ["libcosmic", "iced", "winit", "wayland"] {
        assert!(
            !manifest.contains(forbidden),
            "ghaf-setup-core must not depend on `{forbidden}`"
        );
    }
}
