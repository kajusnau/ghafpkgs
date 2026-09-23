// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::progress::{Phase, ProgressEvent};

#[test]
fn phase_labels_are_user_facing() {
    assert_eq!(Phase::Wipe.label(), "Prepare disk");
    assert_eq!(Phase::WriteImage.label(), "Install Ghaf");
    assert_eq!(Phase::Encryption.label(), "Prepare encryption");
    assert_eq!(Phase::BootEntry.label(), "Create boot entry");
    assert_eq!(Phase::SecureBoot.label(), "Enroll Secure Boot keys");
    assert_eq!(Phase::Wipe.active_label(), "Preparing disk");
    assert_eq!(Phase::WriteImage.active_label(), "Installing Ghaf");
    assert_eq!(Phase::Encryption.active_label(), "Preparing encryption");
    assert_eq!(Phase::BootEntry.active_label(), "Creating boot entry");
    assert_eq!(
        Phase::SecureBoot.active_label(),
        "Enrolling Secure Boot keys"
    );
}

#[test]
fn fractions_are_clamped() {
    let event = ProgressEvent::progress(Phase::WriteImage, 1.7);
    match event {
        ProgressEvent::PhaseProgress { fraction, .. } => assert_eq!(fraction, 1.0),
        other => panic!("expected PhaseProgress, got {other:?}"),
    }
}
