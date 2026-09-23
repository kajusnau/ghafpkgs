// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The vocabulary the running page renders.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Wipe,
    WriteImage,
    Encryption,
    BootEntry,
    SecureBoot,
}

impl Phase {
    /// The step as a to-do, before it has started.
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Wipe => "Prepare disk",
            Phase::WriteImage => "Install Ghaf",
            Phase::Encryption => "Prepare encryption",
            Phase::BootEntry => "Create boot entry",
            Phase::SecureBoot => "Enroll Secure Boot keys",
        }
    }

    /// The step once it has started, kept after it finishes.
    pub fn active_label(&self) -> &'static str {
        match self {
            Phase::Wipe => "Preparing disk",
            Phase::WriteImage => "Installing Ghaf",
            Phase::Encryption => "Preparing encryption",
            Phase::BootEntry => "Creating boot entry",
            Phase::SecureBoot => "Enrolling Secure Boot keys",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    PhaseStarted(Phase),
    PhaseProgress {
        phase: Phase,
        fraction: f32,
    },
    Log(String),
    PhaseFinished(Phase),
    /// `recoverable` is false once the target disk has been modified. The UI
    /// must not offer Retry in that case; see the spec's error handling.
    Failed {
        phase: Phase,
        message: String,
        recoverable: bool,
    },
}

impl ProgressEvent {
    pub fn progress(phase: Phase, fraction: f32) -> Self {
        ProgressEvent::PhaseProgress {
            phase,
            fraction: fraction.clamp(0.0, 1.0),
        }
    }
}

pub type ProgressSender = tokio::sync::mpsc::UnboundedSender<ProgressEvent>;
