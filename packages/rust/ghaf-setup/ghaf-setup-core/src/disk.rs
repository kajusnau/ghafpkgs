// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Block device enumeration and validation.

use crate::proc::{CommandRunner, CoreError};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    pub path: String,
    pub size_bytes: u64,
    pub model: Option<String>,
    /// True for USB sticks and card readers. The installer is usually running
    /// from one of these, so selecting it is the most damaging mistake
    /// available and the UI styles it as such.
    pub removable: bool,
    pub read_only: bool,
}

impl BlockDevice {
    pub fn human_size(&self) -> String {
        let tenths = self.size_bytes * 10 / 1024 / 1024 / 1024;
        if tenths > 0 {
            format!("{}.{} GiB", tenths / 10, tenths % 10)
        } else {
            format!("{} MiB", self.size_bytes / 1024 / 1024)
        }
    }
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Deserialize)]
struct LsblkDevice {
    path: String,
    size: u64,
    #[serde(rename = "type")]
    kind: String,
    rm: bool,
    ro: bool,
    model: Option<String>,
}

/// Lists the disks the installer may write to.
///
/// `boot_device` is the medium the installer itself booted from, when known;
/// it is filtered out entirely rather than merely warned about.
pub async fn enumerate(
    runner: &dyn CommandRunner,
    boot_device: Option<&str>,
) -> Result<Vec<BlockDevice>, CoreError> {
    let output = runner
        .run(
            "lsblk",
            &[
                "--json",
                "--bytes",
                "--nodeps",
                "-o",
                "PATH,SIZE,TYPE,RM,RO,MODEL",
            ],
        )
        .await?;

    let parsed: LsblkOutput = serde_json::from_str(&output.stdout)
        .map_err(|e| CoreError::Parse(format!("lsblk: {e}")))?;

    Ok(parsed
        .blockdevices
        .into_iter()
        // "disk" excludes partitions, loop devices and optical drives.
        .filter(|d| d.kind == "disk")
        .filter(|d| Some(d.path.as_str()) != boot_device)
        .map(|d| BlockDevice {
            path: d.path,
            size_bytes: d.size,
            model: d.model.filter(|m| !m.trim().is_empty()),
            removable: d.rm,
            read_only: d.ro,
        })
        .collect())
}

/// Guards against a device path reaching an argv as anything but a device.
/// `proc` already forbids shell interpolation; this is the second layer.
pub fn validate_path(path: &str) -> Result<(), CoreError> {
    let rest = path.strip_prefix("/dev/").unwrap_or_default();
    let valid = !rest.is_empty()
        && rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));

    if valid {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "not a valid device path: {path}"
        )))
    }
}
