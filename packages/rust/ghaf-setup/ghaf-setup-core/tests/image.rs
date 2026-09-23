// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::install::image::parse_bmaptool_progress;
use ghaf_setup_core::install::{ImageSource, write_image};
use ghaf_setup_core::proc::{CoreError, RecordingRunner};
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn classifies_the_image_source() {
    assert!(matches!(
        ImageSource::parse("/iso/ghaf-image"),
        ImageSource::LocalDir(_)
    ));
    assert!(matches!(
        ImageSource::parse("http://192.0.2.1:8080/ghaf-image"),
        ImageSource::Url(_)
    ));
    assert!(matches!(
        ImageSource::parse("https://example.test/ghaf-image"),
        ImageSource::Url(_)
    ));
}

#[tokio::test]
async fn local_install_passes_paths_to_bmaptool() {
    let runner = RecordingRunner::new();
    runner.push_ok("");
    let (tx, _rx) = unbounded_channel();

    write_image(
        &runner,
        &ImageSource::LocalDir("/iso/ghaf-image".into()),
        "/dev/sda",
        &tx,
    )
    .await
    .unwrap();

    let (program, args) = runner.calls().into_iter().next().unwrap();
    assert_eq!(program, "bmaptool");
    assert_eq!(
        args,
        vec![
            "copy",
            "--bmap",
            "/iso/ghaf-image/ghaf-image.bmap",
            "--",
            "/iso/ghaf-image/ghaf-image.raw.zst",
            "/dev/sda",
        ]
    );
}

#[tokio::test]
async fn netboot_install_hands_bmaptool_the_urls() {
    let runner = RecordingRunner::new();
    runner.push_ok("");
    let (tx, _rx) = unbounded_channel();

    write_image(
        &runner,
        &ImageSource::Url("http://192.0.2.1:8080/ghaf-image".into()),
        "/dev/sda",
        &tx,
    )
    .await
    .unwrap();

    let (_, args) = runner.calls().into_iter().next().unwrap();
    assert_eq!(args[2], "http://192.0.2.1:8080/ghaf-image/ghaf-image.bmap");
    assert_eq!(args[3], "--");
    assert_eq!(
        args[4],
        "http://192.0.2.1:8080/ghaf-image/ghaf-image.raw.zst"
    );
}

#[tokio::test]
async fn rejects_an_invalid_device_before_writing() {
    let runner = RecordingRunner::new();
    let (tx, _rx) = unbounded_channel();

    let err = write_image(
        &runner,
        &ImageSource::LocalDir("/iso".into()),
        "/tmp/x",
        &tx,
    )
    .await
    .unwrap_err();

    assert!(matches!(err, CoreError::Validation(_)));
    assert!(runner.calls().is_empty());
}

#[test]
fn reads_percentages_out_of_bmaptool_output() {
    assert_eq!(
        parse_bmaptool_progress("bmaptool: info: 42% copied"),
        Some(0.42)
    );
    assert_eq!(
        parse_bmaptool_progress("bmaptool: info: 100% copied"),
        Some(1.0)
    );
    assert_eq!(
        parse_bmaptool_progress("bmaptool: info: synchronizing"),
        None
    );
    assert_eq!(parse_bmaptool_progress(""), None);
}

#[tokio::test]
async fn a_failed_write_is_unrecoverable() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "bmaptool: error: cannot open /dev/sda");
    let (tx, mut rx) = unbounded_channel();

    let result = write_image(
        &runner,
        &ImageSource::LocalDir("/iso/ghaf-image".into()),
        "/dev/sda",
        &tx,
    )
    .await;
    drop(tx);

    assert!(result.is_err());

    let mut saw_unrecoverable = false;
    while let Some(event) = rx.recv().await {
        if let ProgressEvent::Failed {
            recoverable, phase, ..
        } = event
        {
            assert_eq!(phase, Phase::WriteImage);
            assert!(!recoverable, "a half-written disk must never offer Retry");
            saw_unrecoverable = true;
        }
    }
    assert!(saw_unrecoverable);
}
