// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::progress::{Phase, ProgressEvent};

#[test]
fn phase_labels_are_user_facing() {
    assert_eq!(Phase::Wipe.label(), "Preparing disk");
    assert_eq!(Phase::WriteImage.label(), "Writing Ghaf image");
    assert_eq!(Phase::Encryption.label(), "Preparing encryption");
    assert_eq!(Phase::BootEntry.label(), "Creating boot entry");
    assert_eq!(Phase::SecureBoot.label(), "Enrolling Secure Boot keys");
}

#[test]
fn fractions_are_clamped() {
    let event = ProgressEvent::progress(Phase::WriteImage, 1.7);
    match event {
        ProgressEvent::PhaseProgress { fraction, .. } => assert_eq!(fraction, 1.0),
        other => panic!("expected PhaseProgress, got {other:?}"),
    }
}
