// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_installer_gui::boot_device::{Unresolved, derive_boot_device, resolve_boot_device};
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
fn no_source_means_no_answer() {
    assert_eq!(
        resolve_boot_device("/iso/ghaf-image", None, Ok(Some("sda"))),
        Ok(None)
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
