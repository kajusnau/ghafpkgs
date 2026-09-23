// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::install::image::parse_bmaptool_progress;
use ghaf_setup_core::install::{ImageSource, write_image};
use ghaf_setup_core::proc::{CommandRunner, CoreError, Output, RecordingRunner};
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
        args[..3],
        ["copy", "--bmap", "/iso/ghaf-image/ghaf-image.bmap"]
    );
    assert_eq!(
        args[5..],
        ["--", "/iso/ghaf-image/ghaf-image.raw.zst", "/dev/sda"]
    );
}

#[tokio::test]
async fn bmaptool_reports_progress_into_a_pipe() {
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

    let (_, args) = runner.calls().into_iter().next().unwrap();
    assert_eq!(args[3], "--psplash-pipe");
    assert!(
        !std::path::Path::new(&args[4]).exists(),
        "the pipe is removed once bmaptool is done"
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
    assert_eq!(args[5], "--");
    assert_eq!(
        args[6],
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
    assert_eq!(parse_bmaptool_progress("PROGRESS 42"), Some(0.42));
    assert_eq!(parse_bmaptool_progress("PROGRESS 100\n"), Some(1.0));
    assert_eq!(parse_bmaptool_progress("MSG synchronizing"), None);
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

/// Writes to `--psplash-pipe` the way bmaptool does: a fresh non-blocking
/// open per update.
struct PsplashWriter;

#[async_trait::async_trait]
impl CommandRunner for PsplashWriter {
    async fn run(&self, _: &str, args: &[&str]) -> Result<Output, CoreError> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        for percent in [25, 50] {
            std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(args[4])?
                .write_all(format!("PROGRESS {percent}\n").as_bytes())?;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok(Output::default())
    }
}

#[tokio::test]
async fn progress_written_to_the_pipe_reaches_the_ui() {
    let (tx, mut rx) = unbounded_channel();

    write_image(
        &PsplashWriter,
        &ImageSource::LocalDir("/iso/ghaf-image".into()),
        "/dev/sda",
        &tx,
    )
    .await
    .unwrap();
    drop(tx);

    let mut fractions = Vec::new();
    while let Some(event) = rx.recv().await {
        if let ProgressEvent::PhaseProgress { fraction, .. } = event {
            fractions.push(fraction);
        }
    }
    assert_eq!(fractions, [0.25, 0.5]);
}
