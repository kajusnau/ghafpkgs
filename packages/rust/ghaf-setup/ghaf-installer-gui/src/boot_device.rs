// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Finding the disk the installer itself booted from, so `disk::enumerate`
//! can exclude it from the list of install targets.
//!
//! Mirrors the shell installer's `boot_device()`
//! (`ghaf-installer-tui/ghaf-installer-lib.sh`): resolve the block device
//! backing `IMG_PATH` via `findmnt`, then walk up to its parent disk via
//! `lsblk -no pkname`. Both steps go through lsblk-family tools, so the
//! result is spelled the way `disk::enumerate`'s own `lsblk --json` query
//! spells its `PATH` column -- unlike `/proc/cmdline`, a `/dev/disk/by-*`
//! symlink, or `/proc/mounts`, any of which can differ in spelling and
//! silently defeat the exclusion.

use ghaf_setup_core::CommandRunner;
use ghaf_setup_core::disk::validate_path;

/// The boot medium could not be identified; the caller must surface this
/// rather than silently excluding nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unresolved;

/// The pure decision, once `findmnt`'s source device and `lsblk`'s parent
/// kernel name (if any) are known. Split out from the I/O so it can be
/// tested without a `CommandRunner`.
///
/// `img_path` is the raw, unparsed value (before `ImageSource::parse`):
/// only the raw string reliably preserves the `http(s)://` scheme check.
///
/// `pkname` is `Err(Unresolved)` when the `lsblk` lookup itself failed --
/// distinct from `Ok(None)`, which means "no parent, already a whole disk".
pub fn resolve_boot_device(
    img_path: &str,
    source: Option<&str>,
    pkname: Result<Option<&str>, Unresolved>,
) -> Result<Option<String>, Unresolved> {
    if img_path.starts_with("http://") || img_path.starts_with("https://") {
        // Netboot: bmaptool reads the image straight off the network, so
        // there is no physical installer medium to exclude. `None` here
        // means "exclude nothing", which is the correct answer, not a
        // fallback -- every local disk is a legitimate target.
        return Ok(None);
    }

    let Some(source) = source else {
        return Ok(None);
    };

    let candidate = match pkname {
        Err(Unresolved) => return Err(Unresolved),
        Ok(Some(pk)) if !pk.is_empty() => format!("/dev/{pk}"),
        Ok(_) => source.to_string(),
    };

    validate_path(&candidate).map_err(|_| Unresolved)?;
    Ok(Some(candidate))
}

/// Runs `findmnt`/`lsblk` to answer `resolve_boot_device`'s question for the
/// live system. `Err(Unresolved)` means the boot medium could not be identified at
/// all -- the caller must surface that rather than silently excluding
/// nothing, since an installer that offers its own boot medium as a target
/// is the worst mistake this application can make.
pub async fn derive_boot_device(
    runner: &dyn CommandRunner,
    img_path: &str,
) -> Result<Option<String>, Unresolved> {
    if img_path.starts_with("http://") || img_path.starts_with("https://") {
        return Ok(None);
    }

    let target = if img_path.is_empty() { "/" } else { img_path };

    let mount = runner
        .run("findmnt", &["-n", "-o", "SOURCE", "--target", target])
        .await
        .map_err(|_| Unresolved)?;
    let source = mount.stdout.trim();
    if source.is_empty() {
        return Ok(None);
    }

    let pkname: Result<Option<String>, Unresolved> =
        match runner.run("lsblk", &["-no", "pkname", source]).await {
            Ok(output) => Ok(output.stdout.lines().next().map(|pk| pk.trim().to_string())),
            Err(_) => Err(Unresolved),
        };

    match pkname {
        Ok(pk) => resolve_boot_device(img_path, Some(source), Ok(pk.as_deref())),
        Err(Unresolved) => resolve_boot_device(img_path, Some(source), Err(Unresolved)),
    }
}
