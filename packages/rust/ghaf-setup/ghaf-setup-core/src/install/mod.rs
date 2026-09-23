// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

pub mod boot;
pub mod image;
pub mod wipe;

pub use boot::{
    create_boot_entry, enroll_secureboot, find_esp, in_setup_mode, write_encrypt_marker,
};
pub use image::{ImageSource, write_image};
pub use wipe::wipe_device;

use crate::proc::{CommandRunner, CoreError};
use crate::progress::{Phase, ProgressEvent, ProgressSender};

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub device: String,
    pub source: ImageSource,
    /// Writes the ESP marker the initrd service looks for. The passphrase
    /// prompt happens on first boot, not here.
    pub encrypt: bool,
    pub secure_boot: bool,
    pub wipe_only: bool,
}

impl InstallRequest {
    /// The phases this request runs, in order: what the progress page lists.
    pub fn phases(&self) -> Vec<Phase> {
        if self.wipe_only {
            return vec![Phase::Wipe];
        }
        let mut phases = vec![Phase::Wipe, Phase::WriteImage];
        if self.encrypt {
            phases.push(Phase::Encryption);
        }
        phases.push(Phase::BootEntry);
        if self.secure_boot {
            phases.push(Phase::SecureBoot);
        }
        phases
    }
}

pub async fn install(
    runner: &dyn CommandRunner,
    request: &InstallRequest,
    progress: &ProgressSender,
) -> Result<(), CoreError> {
    wipe_device(runner, &request.device, progress).await?;

    if request.wipe_only {
        return Ok(());
    }

    write_image(runner, &request.source, &request.device, progress).await?;

    // The ESP lookup opens whichever phase comes next, so a missing ESP is not
    // reported as a failed image write.
    let next = if request.encrypt {
        Phase::Encryption
    } else {
        Phase::BootEntry
    };
    let _ = progress.send(ProgressEvent::PhaseStarted(next));
    let esp = boot::find_esp(runner, &request.device).await?;

    if request.encrypt {
        boot::write_encrypt_marker(runner, &esp).await?;
        let _ = progress.send(ProgressEvent::PhaseFinished(Phase::Encryption));
        let _ = progress.send(ProgressEvent::PhaseStarted(Phase::BootEntry));
    }

    boot::create_boot_entry(runner, &request.device, &esp, progress).await?;

    if request.secure_boot {
        boot::enroll_secureboot(runner, progress).await?;
    }

    Ok(())
}
