// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! EFI boot entry, the deferred-encryption marker, and Secure Boot keys.

use crate::disk::validate_path;
use crate::proc::{CommandRunner, CoreError};
use crate::progress::{Phase, ProgressEvent, ProgressSender};
use serde::Deserialize;

/// The EFI System Partition GUID. Matching on type rather than on a label is
/// what makes this work against images whose PARTLABEL changes.
const ESP_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";

const ESP_MOUNT: &str = "/mnt/esp";
const ENCRYPT_MARKER: &str = "/mnt/esp/.ghaf-installer-encrypt";
const LOADER_PATH: &str = "\\EFI\\BOOT\\BOOTX64.EFI";
const BOOT_LABEL: &str = "Ghaf";

/// A freshly written partition table appears only once the kernel re-reads it
/// and udev settles; the shell installer retries the same way.
const ESP_ATTEMPTS: u32 = 5;
const ESP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Deserialize)]
struct Partitions {
    blockdevices: Vec<Partition>,
}

#[derive(Deserialize)]
struct Partition {
    path: String,
    parttype: Option<String>,
}

pub async fn find_esp(runner: &dyn CommandRunner, device: &str) -> Result<String, CoreError> {
    validate_path(device)?;

    for attempt in 1..=ESP_ATTEMPTS {
        // Best-effort: the lookup below is what decides.
        let _ = runner.run("partprobe", &[device]).await;
        let _ = runner.run("udevadm", &["settle"]).await;

        let output = runner
            .run(
                "lsblk",
                &["--json", "--list", "-o", "PATH,PARTTYPE", device],
            )
            .await?;

        let parsed: Partitions = serde_json::from_str(&output.stdout)
            .map_err(|e| CoreError::Parse(format!("lsblk: {e}")))?;

        let esp = parsed.blockdevices.into_iter().find(|p| {
            p.parttype
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(ESP_GUID))
        });
        if let Some(esp) = esp {
            return Ok(esp.path);
        }
        if attempt < ESP_ATTEMPTS {
            tokio::time::sleep(ESP_RETRY_DELAY).await;
        }
    }

    Err(CoreError::Parse(format!(
        "no EFI system partition on {device}"
    )))
}

/// Leaves the flag the initrd's first-boot service looks for. The encryption
/// prompt itself runs there, on gum -- see the design spec's Scope.
pub async fn write_encrypt_marker(runner: &dyn CommandRunner, esp: &str) -> Result<(), CoreError> {
    validate_path(esp)?;

    runner.run("mkdir", &["-p", ESP_MOUNT]).await?;
    runner.run("mount", &[esp, ESP_MOUNT]).await?;

    let result = runner.run("touch", &[ENCRYPT_MARKER]).await;

    // Unmount whether or not the marker was written: leaving the ESP mounted
    // makes the reboot that follows dirty the filesystem.
    let unmount = runner.run("umount", &[ESP_MOUNT]).await;

    result?;
    unmount?;
    Ok(())
}

/// Extracts the partition number from `esp` given the whole-disk `device` it
/// belongs to, by stripping the known `device` prefix rather than guessing
/// at device-naming families (nvme/mmcblk's `p` separator vs. sd's none).
pub fn partition_number(device: &str, esp: &str) -> Result<String, CoreError> {
    let no_partition_number =
        || CoreError::Parse(format!("cannot read a partition number from {esp}"));

    let remainder = esp.strip_prefix(device).ok_or_else(no_partition_number)?;
    let digits = remainder.strip_prefix('p').unwrap_or(remainder);

    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(no_partition_number());
    }

    Ok(digits.to_string())
}

pub async fn create_boot_entry(
    runner: &dyn CommandRunner,
    device: &str,
    esp: &str,
    progress: &ProgressSender,
) -> Result<(), CoreError> {
    validate_path(device)?;
    validate_path(esp)?;

    let part_number = partition_number(device, esp)?;

    runner
        .run(
            "efibootmgr",
            &[
                "--create",
                "--disk",
                device,
                "--part",
                &part_number,
                "--loader",
                LOADER_PATH,
                "--label",
                BOOT_LABEL,
            ],
        )
        .await?;

    let _ = progress.send(ProgressEvent::PhaseFinished(Phase::BootEntry));
    Ok(())
}

/// Whether the firmware will accept new Secure Boot keys. Any failure to read
/// the variable answers "no": offering enrollment that cannot work is worse
/// than not offering it.
pub async fn in_setup_mode(runner: &dyn CommandRunner) -> bool {
    match runner.run("efi-readvar", &["-v", "SetupMode"]).await {
        Ok(output) => output.stdout.contains("SetupMode: 1"),
        Err(_) => false,
    }
}

pub async fn enroll_secureboot(
    runner: &dyn CommandRunner,
    progress: &ProgressSender,
) -> Result<(), CoreError> {
    let _ = progress.send(ProgressEvent::PhaseStarted(Phase::SecureBoot));

    match runner
        .run(
            "efi-updatevar",
            &["-f", "/etc/secureboot/keys/PK.auth", "PK"],
        )
        .await
    {
        Ok(_) => {
            let _ = progress.send(ProgressEvent::PhaseFinished(Phase::SecureBoot));
            Ok(())
        }
        Err(error) => {
            // The system is installed and bootable at this point; only key
            // enrollment failed, so this one is safe to retry or skip.
            let _ = progress.send(ProgressEvent::Failed {
                phase: Phase::SecureBoot,
                message: error.to_string(),
                recoverable: true,
            });
            Err(error)
        }
    }
}
