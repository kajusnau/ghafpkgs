// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::install::wipe_device;
use ghaf_setup_core::proc::{CoreError, RecordingRunner};
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use tokio::sync::mpsc::unbounded_channel;

fn runner_with_successful_wipe() -> RecordingRunner {
    let runner = RecordingRunner::new();
    // pvs: one PV on our disk's partition, one on an unrelated disk
    runner.push_ok("/dev/sda2  ghaf_vg\n/dev/sdb1  other_vg\n");
    runner.push_ok(""); // vgchange -an
    runner.push_ok(""); // pvremove
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    runner
}

#[tokio::test]
async fn wipes_in_the_required_order() {
    let runner = runner_with_successful_wipe();
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        programs,
        vec!["pvs", "vgchange", "pvremove", "wipefs", "udevadm"]
    );
}

#[tokio::test]
async fn tears_down_only_the_pv_on_the_target_disk() {
    let runner = runner_with_successful_wipe();
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let calls = runner.calls();
    let vgchange = calls.iter().find(|(p, _)| p == "vgchange").unwrap();
    assert_eq!(vgchange.1, vec!["-an", "--", "ghaf_vg"]);

    let pvremove = calls.iter().find(|(p, _)| p == "pvremove").unwrap();
    assert_eq!(
        pvremove.1,
        vec!["--force", "--force", "--yes", "--", "/dev/sda2"]
    );
}

#[tokio::test]
async fn passes_the_device_to_wipefs_with_all() {
    let runner = runner_with_successful_wipe();
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let wipefs = runner
        .calls()
        .into_iter()
        .find(|(p, _)| p == "wipefs")
        .unwrap();
    assert_eq!(wipefs.1, vec!["--all", "--force", "/dev/sda"]);
}

#[tokio::test]
async fn rejects_an_invalid_device_before_running_anything() {
    let runner = RecordingRunner::new();
    let (tx, _rx) = unbounded_channel();

    let err = wipe_device(&runner, "/tmp/evil", &tx).await.unwrap_err();

    assert!(matches!(err, CoreError::Validation(_)));
    assert!(
        runner.calls().is_empty(),
        "nothing may run for an invalid device"
    );
}

#[tokio::test]
async fn emits_phase_started_and_finished() {
    let runner = runner_with_successful_wipe();
    let (tx, mut rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();
    drop(tx);

    let mut events = Vec::new();
    while let Some(e) = rx.recv().await {
        events.push(e);
    }
    assert!(matches!(
        events.first(),
        Some(ProgressEvent::PhaseStarted(Phase::Wipe))
    ));
    assert!(matches!(
        events.last(),
        Some(ProgressEvent::PhaseFinished(Phase::Wipe))
    ));
}

#[tokio::test]
async fn no_volume_groups_is_not_an_error() {
    let runner = RecordingRunner::new();
    runner.push_ok(""); // pvs: no VGs
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        programs,
        vec!["pvs", "wipefs", "udevadm"],
        "LVM teardown is skipped"
    );
}

#[tokio::test]
async fn a_failing_pvs_is_not_an_error_and_wipe_still_proceeds() {
    let runner = RecordingRunner::new();
    runner.push_fail(5, "device is not a physical volume"); // pvs
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        programs,
        vec!["pvs", "wipefs", "udevadm"],
        "a non-LVM disk must still be wiped"
    );
}

#[tokio::test]
async fn a_lookalike_disk_name_is_not_matched_as_a_partition() {
    let runner = RecordingRunner::new();
    // /dev/sdaa1 has /dev/sda as a string prefix but is a different disk.
    runner.push_ok("/dev/sdaa1  other_vg\n");
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        programs,
        vec!["pvs", "wipefs", "udevadm"],
        "a PV on an unrelated disk must not be torn down"
    );
}

#[tokio::test]
async fn a_longer_disk_name_ending_in_p_is_not_matched_as_a_partition() {
    let runner = RecordingRunner::new();
    // /dev/sdap is a different disk (the 42nd SCSI disk); its partition 1
    // must not be mistaken for a `p`-separated partition of /dev/sda.
    runner.push_ok("/dev/sdap1  other_vg\n");
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(programs, vec!["pvs", "wipefs", "udevadm"]);
}

#[tokio::test]
async fn an_nvme_namespace_ten_is_not_matched_as_namespace_ones_partition() {
    let runner = RecordingRunner::new();
    runner.push_ok("/dev/nvme0n10  other_vg\n");
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/nvme0n1", &tx).await.unwrap();

    let programs: Vec<_> = runner.calls().into_iter().map(|(p, _)| p).collect();
    assert_eq!(programs, vec!["pvs", "wipefs", "udevadm"]);
}

#[tokio::test]
async fn an_orphan_pv_with_no_volume_group_is_still_removed() {
    let runner = RecordingRunner::new();
    runner.push_ok("/dev/sda2\n"); // pvs: PV with no VG column
    runner.push_ok(""); // pvremove
    runner.push_ok(""); // wipefs
    runner.push_ok(""); // udevadm settle
    let (tx, _rx) = unbounded_channel();

    wipe_device(&runner, "/dev/sda", &tx).await.unwrap();

    let calls = runner.calls();
    assert!(calls.iter().all(|(p, _)| p != "vgchange"));
    let pvremove = calls.iter().find(|(p, _)| p == "pvremove").unwrap();
    assert_eq!(
        pvremove.1,
        vec!["--force", "--force", "--yes", "--", "/dev/sda2"]
    );
}

#[tokio::test]
async fn a_failing_wipefs_is_reported_as_unrecoverable() {
    let runner = RecordingRunner::new();
    runner.push_ok(""); // pvs: no VGs
    runner.push_fail(1, "device or resource busy"); // wipefs
    let (tx, mut rx) = unbounded_channel();

    let err = wipe_device(&runner, "/dev/sda", &tx).await.unwrap_err();
    assert!(matches!(err, CoreError::Command { .. }));
    drop(tx);

    let mut events = Vec::new();
    while let Some(e) = rx.recv().await {
        events.push(e);
    }
    assert!(matches!(
        events.last(),
        Some(ProgressEvent::Failed {
            recoverable: false,
            ..
        })
    ));
}
