// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::disk::{BlockDevice, enumerate, validate_path};
use ghaf_setup_core::proc::{CoreError, RecordingRunner};

const FIXTURE: &str = include_str!("fixtures/lsblk.json");

#[tokio::test]
async fn lists_disks_and_skips_partitions_and_rom() {
    let runner = RecordingRunner::new();
    runner.push_ok(FIXTURE);

    let devices = enumerate(&runner, None).await.unwrap();

    let paths: Vec<_> = devices.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(paths, vec!["/dev/nvme0n1", "/dev/sda"]);
}

#[tokio::test]
async fn marks_usb_media_removable() {
    let runner = RecordingRunner::new();
    runner.push_ok(FIXTURE);

    let devices = enumerate(&runner, None).await.unwrap();

    assert!(!devices[0].removable);
    assert!(
        devices[1].removable,
        "the USB stick must be flagged removable"
    );
}

#[tokio::test]
async fn excludes_the_boot_device() {
    let runner = RecordingRunner::new();
    runner.push_ok(FIXTURE);

    let devices = enumerate(&runner, Some("/dev/sda")).await.unwrap();

    let paths: Vec<_> = devices.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["/dev/nvme0n1"],
        "installing over our own media must not be offered"
    );
}

#[tokio::test]
async fn queries_lsblk_with_json_and_bytes() {
    let runner = RecordingRunner::new();
    runner.push_ok(FIXTURE);

    enumerate(&runner, None).await.unwrap();

    let (program, args) = runner.calls().into_iter().next().unwrap();
    assert_eq!(program, "lsblk");
    assert!(args.contains(&"--json".to_string()));
    assert!(args.contains(&"--bytes".to_string()));
}

#[test]
fn human_size_is_readable() {
    let device = BlockDevice {
        path: "/dev/sda".into(),
        size_bytes: 30_765_219_840,
        model: None,
        removable: true,
        read_only: false,
    };
    assert_eq!(device.human_size(), "28.6 GiB");
}

#[test]
fn human_size_sub_gib_uses_mib() {
    let device = BlockDevice {
        path: "/dev/sda".into(),
        size_bytes: 52_428_800, // 50 MiB, well under the 1/10 GiB tenths threshold
        model: None,
        removable: false,
        read_only: false,
    };
    assert_eq!(device.human_size(), "50 MiB");
}

#[test]
fn human_size_exact_one_gib_boundary() {
    let device = BlockDevice {
        path: "/dev/sda".into(),
        size_bytes: 1024 * 1024 * 1024,
        model: None,
        removable: false,
        read_only: false,
    };
    assert_eq!(device.human_size(), "1.0 GiB");
}

#[test]
fn validate_rejects_paths_outside_dev() {
    assert!(validate_path("/dev/sda").is_ok());
    assert!(validate_path("/dev/nvme0n1").is_ok());
    assert!(validate_path("/tmp/evil").is_err());
    assert!(validate_path("/dev/../tmp/evil").is_err());
    assert!(validate_path("/dev/sda; rm -rf /").is_err());
}

#[test]
fn validate_rejects_dev_directory_itself() {
    // `/dev/.` resolves to the `/dev` directory, not a device. A validated
    // path is fed straight into argv for wipefs/pvremove/bmaptool, so this
    // must never pass.
    assert!(validate_path("/dev/.").is_err());
    assert!(validate_path("/dev/..").is_err());
}

#[test]
fn validate_rejects_leading_punctuation() {
    assert!(validate_path("/dev/-foo").is_err());
    assert!(validate_path("/dev/_foo").is_err());
}

#[test]
fn validate_accepts_real_device_name_shapes() {
    assert!(validate_path("/dev/dm-0").is_ok());
    assert!(validate_path("/dev/nvme0n1p1").is_ok());
    assert!(validate_path("/dev/mmcblk0p1").is_ok());
    assert!(validate_path("/dev/vda").is_ok());
}

#[test]
fn validate_rejects_whitespace_newline_and_unicode_lookalikes() {
    assert!(validate_path("/dev/sd a").is_err());
    assert!(validate_path("/dev/sda\n").is_err());
    // Cyrillic "а" (U+0430), not ASCII "a".
    assert!(validate_path("/dev/sd\u{0430}").is_err());
}

#[tokio::test]
async fn malformed_output_is_a_parse_error() {
    let runner = RecordingRunner::new();
    runner.push_ok("not json");

    let err = enumerate(&runner, None).await.unwrap_err();
    assert!(matches!(err, CoreError::Parse(_)));
}

#[tokio::test]
async fn whitespace_only_model_is_normalised_to_none() {
    let json = r#"{
        "blockdevices": [
            { "path": "/dev/sda", "size": 1024, "type": "disk", "rm": false, "ro": false, "model": "   " }
        ]
    }"#;
    let runner = RecordingRunner::new();
    runner.push_ok(json);

    let devices = enumerate(&runner, None).await.unwrap();

    assert_eq!(devices[0].model, None);
}
