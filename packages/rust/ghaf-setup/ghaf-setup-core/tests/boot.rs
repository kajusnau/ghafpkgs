// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::install::boot::{
    create_boot_entry, enroll_secureboot, find_esp, in_setup_mode, partition_number,
    write_encrypt_marker,
};
use ghaf_setup_core::proc::{CoreError, RecordingRunner};
use ghaf_setup_core::progress::ProgressEvent;
use tokio::sync::mpsc::unbounded_channel;

const LSBLK_PARTS: &str = r#"{"blockdevices":[
  {"path":"/dev/sda1","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b","partlabel":"ESP"},
  {"path":"/dev/sda2","parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4","partlabel":"root"}
]}"#;

/// One ESP lookup: re-read the table, settle, then list partitions.
fn queue_lookup(runner: &RecordingRunner, lsblk: &str) {
    runner.push_ok(""); // partprobe
    runner.push_ok(""); // udevadm settle
    runner.push_ok(lsblk);
}

#[tokio::test]
async fn finds_the_esp_by_partition_type() {
    let runner = RecordingRunner::new();
    queue_lookup(&runner, LSBLK_PARTS);

    let esp = find_esp(&runner, "/dev/sda").await.unwrap();

    assert_eq!(esp, "/dev/sda1");
    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        programs,
        ["partprobe", "udevadm", "lsblk"],
        "the kernel must re-read the table before the lookup"
    );
}

#[tokio::test(start_paused = true)]
async fn an_esp_that_appears_late_is_still_found() {
    let runner = RecordingRunner::new();
    queue_lookup(&runner, r#"{"blockdevices":[]}"#);
    queue_lookup(&runner, LSBLK_PARTS);

    let esp = find_esp(&runner, "/dev/sda").await.unwrap();

    assert_eq!(esp, "/dev/sda1");
}

#[tokio::test(start_paused = true)]
async fn a_missing_esp_is_a_parse_error_after_five_attempts() {
    let runner = RecordingRunner::new();
    for _ in 0..5 {
        queue_lookup(&runner, r#"{"blockdevices":[]}"#);
    }

    let err = find_esp(&runner, "/dev/sda").await.unwrap_err();

    assert!(matches!(err, CoreError::Parse(_)));
    let lookups = runner.calls().iter().filter(|(p, _)| p == "lsblk").count();
    assert_eq!(lookups, 5);
}

#[tokio::test]
async fn the_encryption_marker_is_written_to_the_esp_and_unmounted() {
    let runner = RecordingRunner::new();
    runner.push_ok(""); // mkdir
    runner.push_ok(""); // mount
    runner.push_ok(""); // touch
    runner.push_ok(""); // umount

    write_encrypt_marker(&runner, "/dev/sda1").await.unwrap();

    let calls = runner.calls();
    let touch = calls.iter().find(|(p, _)| p == "touch").unwrap();
    assert_eq!(touch.1, vec!["/mnt/esp/.ghaf-installer-encrypt"]);
    assert_eq!(
        calls.last().unwrap().0,
        "umount",
        "the ESP must not be left mounted"
    );
}

#[tokio::test]
async fn creates_a_boot_entry_pointing_at_the_ghaf_loader() {
    let runner = RecordingRunner::new();
    runner.push_ok(""); // efibootmgr --create
    let (tx, _rx) = unbounded_channel();

    create_boot_entry(&runner, "/dev/sda", "/dev/sda1", &tx)
        .await
        .unwrap();

    let (program, args) = runner.calls().into_iter().next().unwrap();
    assert_eq!(program, "efibootmgr");
    assert!(args.contains(&"--create".to_string()));
    assert!(args.contains(&"--disk".to_string()));
    assert!(args.contains(&"/dev/sda".to_string()));
    assert!(args.contains(&"--part".to_string()));
    assert!(args.contains(&"1".to_string()));
    assert!(args.contains(&"\\EFI\\BOOT\\BOOTX64.EFI".to_string()));
    assert!(args.contains(&"Ghaf".to_string()));
}

#[tokio::test]
async fn setup_mode_is_read_from_the_efi_variable() {
    let in_setup = RecordingRunner::new();
    in_setup.push_ok("SetupMode: 1");
    assert!(in_setup_mode(&in_setup).await);

    let not_in_setup = RecordingRunner::new();
    not_in_setup.push_ok("SetupMode: 0");
    assert!(!in_setup_mode(&not_in_setup).await);
}

#[tokio::test]
async fn a_missing_efi_variable_means_not_in_setup_mode() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "efi-readvar: variable SetupMode not found");

    assert!(
        !in_setup_mode(&runner).await,
        "absence must never be read as permission to enroll keys"
    );
}

#[tokio::test]
async fn secure_boot_enrollment_failure_is_recoverable() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "efi-updatevar: permission denied");
    let (tx, mut rx) = unbounded_channel();

    let result = enroll_secureboot(&runner, &tx).await;
    drop(tx);

    assert!(result.is_err());

    let mut saw_recoverable = false;
    while let Some(event) = rx.recv().await {
        if let ProgressEvent::Failed { recoverable, .. } = event {
            // The disk is already installed and bootable; only key
            // enrollment failed, so the user can retry or skip.
            assert!(recoverable);
            saw_recoverable = true;
        }
    }
    assert!(saw_recoverable);
}

#[test]
fn partition_number_sda_sda1_is_1() {
    assert_eq!(partition_number("/dev/sda", "/dev/sda1").unwrap(), "1");
}

#[test]
fn partition_number_sda_sda15_is_15() {
    assert_eq!(partition_number("/dev/sda", "/dev/sda15").unwrap(), "15");
}

#[test]
fn partition_number_nvme0n1_p2_is_2() {
    assert_eq!(
        partition_number("/dev/nvme0n1", "/dev/nvme0n1p2").unwrap(),
        "2"
    );
}

#[test]
fn partition_number_mmcblk0_p1_is_1() {
    assert_eq!(
        partition_number("/dev/mmcblk0", "/dev/mmcblk0p1").unwrap(),
        "1"
    );
}

#[test]
fn partition_number_loop0_p1_is_1() {
    assert_eq!(partition_number("/dev/loop0", "/dev/loop0p1").unwrap(), "1");
}

#[test]
fn partition_number_loop0_whole_disk_is_an_error() {
    assert!(
        partition_number("/dev/loop0", "/dev/loop0").is_err(),
        "a whole disk must never be read as a partition"
    );
}

#[test]
fn partition_number_nvme10n1_p13_is_13() {
    assert_eq!(
        partition_number("/dev/nvme10n1", "/dev/nvme10n1p13").unwrap(),
        "13"
    );
}

#[test]
fn partition_number_esp_equal_to_device_is_an_error() {
    assert!(partition_number("/dev/sda", "/dev/sda").is_err());
}

#[test]
fn partition_number_esp_not_prefixed_by_device_is_an_error() {
    assert!(partition_number("/dev/sda", "/dev/sdb1").is_err());
}

#[test]
fn partition_number_non_digit_remainder_is_an_error() {
    assert!(partition_number("/dev/sda", "/dev/sdap1x").is_err());
}
