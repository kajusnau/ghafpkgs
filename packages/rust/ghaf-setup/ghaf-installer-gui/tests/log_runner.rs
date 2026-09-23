// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_installer_gui::log_runner::{LoggingRunner, MAX_OUTPUT_LINES};
use ghaf_setup_core::proc::{CommandRunner, RecordingRunner};
use ghaf_setup_core::progress::ProgressEvent;
use tokio::sync::mpsc::unbounded_channel;

fn logged(rx: &mut tokio::sync::mpsc::UnboundedReceiver<ProgressEvent>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ProgressEvent::Log(line) = event {
            lines.push(line);
        }
    }
    lines
}

#[tokio::test]
async fn each_command_and_its_output_is_logged() {
    let inner = RecordingRunner::new();
    inner.push_ok("/dev/sda: 8 bytes were erased\n");
    let (tx, mut rx) = unbounded_channel();

    LoggingRunner::new(inner, tx)
        .run("wipefs", &["--all", "/dev/sda"])
        .await
        .unwrap();

    assert_eq!(
        logged(&mut rx),
        ["$ wipefs --all /dev/sda", "  /dev/sda: 8 bytes were erased"]
    );
}

#[tokio::test]
async fn a_failure_is_logged() {
    let inner = RecordingRunner::new();
    inner.push_fail(1, "no such device");
    let (tx, mut rx) = unbounded_channel();

    let result = LoggingRunner::new(inner, tx)
        .run("wipefs", &["/dev/sdz"])
        .await;

    assert!(result.is_err(), "the error still reaches the caller");
    assert!(logged(&mut rx)[1].contains("no such device"));
}

#[tokio::test]
async fn long_output_keeps_only_its_tail() {
    let inner = RecordingRunner::new();
    let output: String = (0..100).map(|i| format!("line {i}\n")).collect();
    inner.push_ok(&output);
    let (tx, mut rx) = unbounded_channel();

    LoggingRunner::new(inner, tx)
        .run("lsblk", &[])
        .await
        .unwrap();

    let lines = logged(&mut rx);
    assert_eq!(lines.len(), MAX_OUTPUT_LINES + 2);
    assert_eq!(
        lines[1],
        format!("  … {} more lines", 100 - MAX_OUTPUT_LINES)
    );
    assert_eq!(lines.last().unwrap(), "  line 99");
}
