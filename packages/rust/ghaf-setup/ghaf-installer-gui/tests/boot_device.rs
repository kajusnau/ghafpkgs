// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_installer_gui::boot_device::{
    Unresolved, derive_boot_device, protect_boot_medium, resolve_boot_device,
};
use ghaf_setup_core::disk::BlockDevice;
use ghaf_setup_core::proc::RecordingRunner;

#[test]
fn netboot_excludes_nothing() {
    assert_eq!(
        resolve_boot_device(
            "http://192.0.2.1:8080/ghaf-image",
            Some("/dev/sda1"),
            Ok(Some("sda"))
        ),
        Ok(None),
        "netboot has no physical installer medium to exclude"
    );
    assert_eq!(
        resolve_boot_device(
            "https://example.com/ghaf-image",
            Some("/dev/sda1"),
            Ok(Some("sda"))
        ),
        Ok(None)
    );
}

#[test]
fn a_partition_source_resolves_to_its_parent_disk() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", Some("/dev/sdb1"), Ok(Some("sdb"))),
        Ok(Some("/dev/sdb".to_string()))
    );
}

#[test]
fn a_whole_disk_source_with_no_parent_is_returned_unchanged() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", Some("/dev/loop0"), Ok(None)),
        Ok(Some("/dev/loop0".to_string()))
    );
}

#[test]
fn an_empty_pkname_is_treated_as_no_parent() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", Some("/dev/sdb"), Ok(Some(""))),
        Ok(Some("/dev/sdb".to_string()))
    );
}

#[test]
fn no_source_is_unresolved() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", None, Ok(Some("sda"))),
        Err(Unresolved),
        "a local image with no source must not read as \"nothing to exclude\""
    );
}

#[test]
fn a_failed_lsblk_lookup_is_not_treated_as_no_parent() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", Some("overlay"), Err(Unresolved)),
        Err(Unresolved),
        "an lsblk failure must not fall through as a resolved device"
    );
}

#[test]
fn a_derived_value_that_fails_validation_is_rejected() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", Some("/dev/sda2[/@]"), Ok(None)),
        Err(Unresolved),
        "a btrfs subvolume-qualified source is not a plain device path"
    );
}

#[tokio::test]
async fn derive_reports_an_error_when_lsblk_cannot_find_the_parent() {
    let runner = RecordingRunner::new();
    runner.push_ok("overlay\n"); // findmnt: a live ISO's overlayfs source
    runner.push_fail(1, "overlay: not a block device"); // lsblk -no pkname

    let result = derive_boot_device(&runner, "/iso/ghaf-image").await;

    assert_eq!(
        result,
        Err(Unresolved),
        "an unresolvable boot medium must be surfaced, not silently excluded as-is"
    );
}

#[tokio::test]
async fn derive_resolves_a_partition_source_to_its_parent_disk() {
    let runner = RecordingRunner::new();
    runner.push_ok("/dev/sdb1\n"); // findmnt
    runner.push_ok("sdb\n"); // lsblk -no pkname

    let result = derive_boot_device(&runner, "/iso/ghaf-image").await;

    assert_eq!(result, Ok(Some("/dev/sdb".to_string())));
}

#[tokio::test]
async fn derive_treats_an_empty_mount_source_as_unresolved() {
    let runner = RecordingRunner::new();
    runner.push_ok("\n"); // findmnt

    let result = derive_boot_device(&runner, "/iso/ghaf-image").await;

    assert_eq!(result, Err(Unresolved));
}

fn disk(path: &str, removable: bool) -> BlockDevice {
    BlockDevice {
        path: path.into(),
        size_bytes: 1 << 34,
        model: None,
        removable,
        read_only: false,
        installation_media: false,
        maybe_installation_media: false,
    }
}

#[test]
fn an_unknown_boot_medium_makes_removable_drives_unavailable() {
    let mut devices = vec![disk("/dev/nvme0n1", false), disk("/dev/sda", true)];

    let warning = protect_boot_medium(&mut devices, &Err(Unresolved));

    assert!(warning.is_some());
    assert!(devices[0].available(), "a fixed disk cannot be the stick");
    assert!(!devices[1].available());
}

#[test]
fn a_boot_medium_missing_from_the_list_fails_closed_too() {
    // e.g. a loop device, which is not a "disk" and so never listed.
    let mut devices = vec![disk("/dev/sda", true)];

    let warning = protect_boot_medium(&mut devices, &Ok(Some("/dev/loop0".into())));

    assert!(warning.is_some());
    assert!(!devices[0].available());
}

#[test]
fn a_found_boot_medium_or_netboot_leaves_the_rest_alone() {
    let mut devices = vec![disk("/dev/sda", true), disk("/dev/sdb", true)];
    assert_eq!(
        protect_boot_medium(&mut devices, &Ok(Some("/dev/sdb".into()))),
        None
    );
    assert_eq!(protect_boot_medium(&mut devices, &Ok(None)), None);
    assert!(devices[0].available());
}
