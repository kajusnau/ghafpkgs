// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::proc::{
    CommandRunner, CoreError, RecordingRunner, STDERR_TAIL_BYTES, SystemRunner, tail,
};

#[tokio::test]
async fn recording_runner_records_exact_argv() {
    let runner = RecordingRunner::new();
    runner.push_ok("done");

    let out = runner.run("wipefs", &["--all", "/dev/sda"]).await.unwrap();

    assert_eq!(out.stdout, "done");
    assert_eq!(
        runner.calls(),
        vec![(
            "wipefs".to_string(),
            vec!["--all".to_string(), "/dev/sda".to_string()]
        )]
    );
}

#[tokio::test]
async fn non_zero_exit_becomes_command_error_with_stderr() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "wipefs: /dev/sda: probing initialization failed");

    let err = runner
        .run("wipefs", &["--all", "/dev/sda"])
        .await
        .unwrap_err();

    match err {
        CoreError::Command {
            program,
            status,
            stderr,
            ..
        } => {
            assert_eq!(program, "wipefs");
            assert_eq!(status, Some(1));
            assert!(stderr.contains("probing initialization failed"));
        }
        other => panic!("expected CoreError::Command, got {other:?}"),
    }
}

#[tokio::test]
async fn system_runner_captures_stdout() {
    let out = SystemRunner.run("echo", &["hello"]).await.unwrap();
    assert_eq!(out.stdout.trim(), "hello");
}

#[tokio::test]
async fn system_runner_maps_failure_to_command_error() {
    let err = SystemRunner.run("false", &[]).await.unwrap_err();
    assert!(matches!(
        err,
        CoreError::Command {
            status: Some(1),
            ..
        }
    ));
}

#[test]
fn long_stderr_is_truncated_to_the_tail() {
    let head = "a".repeat(5000);
    let marker = "END-OF-STDERR";
    let full = format!("{head}{marker}");

    let result = tail(&full);

    // Truncated: shorter than the original, capped at STDERR_TAIL_BYTES.
    assert!(result.len() < full.len());
    assert!(result.len() <= STDERR_TAIL_BYTES);
    // It is the TAIL that survives: the marker at the very end is intact,
    // and the start of the original 5000-byte 'a' run is gone (the kept
    // slice does not begin at byte 0 of `full`).
    assert!(result.ends_with(marker));
}

#[test]
fn truncation_does_not_split_a_multibyte_char_at_the_cut_point() {
    // Deliberately position a 4-byte emoji so the naive cut point
    // `len - STDERR_TAIL_BYTES` lands inside it (not on a char boundary):
    // prefix ends at byte 4094, emoji occupies [4094, 4098), and total
    // length 8192 makes the naive cut point `8192 - 4096 = 4096`, which is
    // two bytes into the emoji.
    let prefix = "a".repeat(4094);
    let emoji = "\u{1F600}"; // 4 bytes in UTF-8
    let suffix = "b".repeat(4094);
    let full = format!("{prefix}{emoji}{suffix}");
    assert_eq!(full.len(), 8192);
    let cut = full.len() - STDERR_TAIL_BYTES;
    assert!(
        cut > 4094 && cut < 4098,
        "cut point must land inside the emoji for this test to be meaningful"
    );

    // Must not panic, and the result must be valid UTF-8 (guaranteed by
    // `String`, but the point is that constructing it doesn't panic on a
    // non-boundary slice).
    let result = tail(&full);

    // The emoji must be kept whole, not sliced through, and the suffix
    // that follows it must be fully intact.
    assert!(result.contains(emoji));
    assert!(result.ends_with(&suffix));
}

#[test]
fn stderr_exactly_at_the_limit_is_not_truncated() {
    let full = "c".repeat(STDERR_TAIL_BYTES);
    assert_eq!(full.len(), STDERR_TAIL_BYTES);

    let result = tail(&full);

    // `<=` edge: exactly STDERR_TAIL_BYTES bytes must pass through unchanged.
    assert_eq!(result, full);
}
