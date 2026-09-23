// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::install::{ImageSource, InstallRequest, install};
use ghaf_setup_core::proc::RecordingRunner;
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use tokio::sync::mpsc::unbounded_channel;

fn request() -> InstallRequest {
    InstallRequest {
        device: "/dev/sda".into(),
        source: ImageSource::LocalDir("/iso/ghaf-image".into()),
        encrypt: false,
        secure_boot: false,
        wipe_only: false,
    }
}

#[test]
fn only_the_requested_phases_are_planned() {
    assert_eq!(
        InstallRequest {
            wipe_only: true,
            ..request()
        }
        .phases(),
        [Phase::Wipe]
    );
    assert_eq!(
        request().phases(),
        [Phase::Wipe, Phase::WriteImage, Phase::BootEntry]
    );
    assert_eq!(
        InstallRequest {
            encrypt: true,
            secure_boot: true,
            ..request()
        }
        .phases(),
        [
            Phase::Wipe,
            Phase::WriteImage,
            Phase::Encryption,
            Phase::BootEntry,
            Phase::SecureBoot
        ]
    );
}

fn queue_wipe(runner: &RecordingRunner) {
    runner.push_ok(""); // pvs
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm
}

#[tokio::test]
async fn wipe_only_stops_after_wiping() {
    let runner = RecordingRunner::new();
    queue_wipe(&runner);
    let (tx, _rx) = unbounded_channel();

    install(
        &runner,
        &InstallRequest {
            wipe_only: true,
            ..request()
        },
        &tx,
    )
    .await
    .unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert!(
        !programs.contains(&"bmaptool".to_string()),
        "nothing may be written"
    );
}

#[tokio::test]
async fn a_plain_install_wipes_writes_and_creates_a_boot_entry() {
    let runner = RecordingRunner::new();
    queue_wipe(&runner);
    runner.push_ok(""); // bmaptool
    runner.push_ok(""); // partprobe
    runner.push_ok(""); // udevadm settle
    runner.push_ok(
        r#"{"blockdevices":[{"path":"/dev/sda1","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b"}]}"#,
    );
    runner.push_ok(""); // efibootmgr
    let (tx, _rx) = unbounded_channel();

    install(&runner, &request(), &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert!(programs.contains(&"bmaptool".to_string()));
    assert!(programs.contains(&"efibootmgr".to_string()));
    assert!(
        !programs.contains(&"touch".to_string()),
        "no marker when encryption is off"
    );
}

#[tokio::test]
async fn encryption_writes_the_marker_before_the_boot_entry() {
    let runner = RecordingRunner::new();
    queue_wipe(&runner);
    runner.push_ok(""); // bmaptool
    runner.push_ok(""); // partprobe
    runner.push_ok(""); // udevadm settle
    runner.push_ok(
        r#"{"blockdevices":[{"path":"/dev/sda1","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b"}]}"#,
    );
    runner.push_ok(""); // mkdir
    runner.push_ok(""); // mount
    runner.push_ok(""); // touch
    runner.push_ok(""); // umount
    runner.push_ok(""); // efibootmgr
    let (tx, mut rx) = unbounded_channel();

    install(
        &runner,
        &InstallRequest {
            encrypt: true,
            ..request()
        },
        &tx,
    )
    .await
    .unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    let marker = programs.iter().position(|p| p == "touch").unwrap();
    let entry = programs.iter().position(|p| p == "efibootmgr").unwrap();
    assert!(marker < entry);

    let mut started = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ProgressEvent::PhaseStarted(phase) = event {
            started.push(phase);
        }
    }
    assert_eq!(
        started,
        [
            Phase::Wipe,
            Phase::WriteImage,
            Phase::Encryption,
            Phase::BootEntry
        ],
        "encryption shows as its own step"
    );
}

#[tokio::test]
async fn a_failed_write_stops_the_install() {
    let runner = RecordingRunner::new();
    queue_wipe(&runner);
    runner.push_fail(1, "bmaptool: error: short write");
    let (tx, _rx) = unbounded_channel();

    let result = install(&runner, &request(), &tx).await;

    assert!(result.is_err());
    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert!(
        !programs.contains(&"efibootmgr".to_string()),
        "no boot entry for a failed image"
    );
}

#[tokio::test(start_paused = true)]
async fn a_missing_esp_fails_in_the_boot_entry_phase() {
    let runner = RecordingRunner::new();
    queue_wipe(&runner);
    runner.push_ok(""); // bmaptool
    for _ in 0..5 {
        runner.push_ok(""); // partprobe
        runner.push_ok(""); // udevadm settle
        runner.push_ok(r#"{"blockdevices":[]}"#);
    }
    let (tx, mut rx) = unbounded_channel();

    assert!(install(&runner, &request(), &tx).await.is_err());

    let mut last_started = None;
    while let Ok(event) = rx.try_recv() {
        if let ProgressEvent::PhaseStarted(phase) = event {
            last_started = Some(phase);
        }
    }
    assert_eq!(
        last_started,
        Some(Phase::BootEntry),
        "the image was written; a missing ESP must not read as a failed write"
    );
}
