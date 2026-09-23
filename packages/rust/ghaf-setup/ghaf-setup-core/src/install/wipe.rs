// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Disk teardown, mirroring the order the shell installer established:
//! a stale volume group holding the device open makes every later step fail
//! in a confusing way, so LVM comes off first.

use crate::disk::validate_path;
use crate::proc::{CommandRunner, CoreError};
use crate::progress::{Phase, ProgressEvent, ProgressSender};

/// Whether `pv_name` is `device` itself or one of its partitions, not just a
/// string with `device` as a prefix (`/dev/sda` vs. `/dev/sdaa1`/`nvme0n10`).
fn is_on_device(pv_name: &str, device: &str) -> bool {
    if pv_name == device {
        return true;
    }
    let Some(rest) = pv_name.strip_prefix(device) else {
        return false;
    };
    let digits = if device
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_digit())
    {
        // nvme0n1, mmcblk0, loop0: the disk name ends in a digit, so a
        // partition must carry the p separator to be unambiguous.
        match rest.strip_prefix('p') {
            Some(digits) => digits,
            None => return false,
        }
    } else {
        // sda, vda, hda: partitions are bare digits; a leading p means this
        // is a different, longer disk name (sdap1 is not a partition of sda).
        rest
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

pub async fn wipe_device(
    runner: &dyn CommandRunner,
    device: &str,
    progress: &ProgressSender,
) -> Result<(), CoreError> {
    if let Err(error) = validate_path(device) {
        // `recoverable` is reserved for a Secure Boot enrollment retry; a
        // rejected device path is a distinct case the GUI reads off the
        // absence of any `PhaseStarted` event instead.
        let _ = progress.send(ProgressEvent::Failed {
            phase: Phase::Wipe,
            message: error.to_string(),
            recoverable: false,
        });
        return Err(error);
    }
    let _ = progress.send(ProgressEvent::PhaseStarted(Phase::Wipe));

    // `pvs` exits non-zero for "no volume groups", not an error; enumerate
    // with no device filter since a PV lives on a partition, not the disk.
    if let Ok(pvs) = runner
        .run("pvs", &["--noheadings", "-o", "pv_name,vg_name"])
        .await
    {
        let mut pv_paths = Vec::new();
        let mut groups = Vec::new();

        for line in pvs.stdout.lines() {
            let mut fields = line.split_whitespace();
            let Some(pv_name) = fields.next() else {
                continue;
            };
            let vg_name = fields.next().unwrap_or("");
            if !is_on_device(pv_name, device) || validate_path(pv_name).is_err() {
                continue;
            }
            pv_paths.push(pv_name.to_string());
            if !vg_name.is_empty() && !groups.contains(&vg_name.to_string()) {
                groups.push(vg_name.to_string());
            }
        }

        for group in &groups {
            let _ = progress.send(ProgressEvent::Log(format!(
                "Deactivating volume group {group}"
            )));
            let _ = runner.run("vgchange", &["-an", "--", group]).await;
        }

        if !pv_paths.is_empty() {
            let mut args = vec!["--force", "--force", "--yes", "--"];
            args.extend(pv_paths.iter().map(String::as_str));
            let _ = runner.run("pvremove", &args).await;
        }
    }

    let _ = progress.send(ProgressEvent::Log(format!("Wiping signatures on {device}")));
    if let Err(error) = runner.run("wipefs", &["--all", "--force", device]).await {
        let _ = progress.send(ProgressEvent::Failed {
            phase: Phase::Wipe,
            message: error.to_string(),
            recoverable: false,
        });
        return Err(error);
    }

    // Without this the partition table the image writes can be read back
    // stale, and the ESP lookup that follows finds nothing.
    if let Err(error) = runner.run("udevadm", &["settle"]).await {
        let _ = progress.send(ProgressEvent::Failed {
            phase: Phase::Wipe,
            message: error.to_string(),
            recoverable: false,
        });
        return Err(error);
    }

    let _ = progress.send(ProgressEvent::PhaseFinished(Phase::Wipe));
    Ok(())
}
