// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The single audited entry point for every external command.
//!
//! These commands erase disks and create accounts. Keeping them behind one
//! trait gives one place to audit argv construction, one place that logs, and
//! a recording fake so tests can assert exactly what would have been run.

use async_trait::async_trait;
use std::sync::Mutex;

/// The tail of stderr kept on a failed command. Enough for a useful message,
/// bounded so a runaway tool cannot blow up an error value.
pub const STDERR_TAIL_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("`{program}` failed with status {status:?}: {stderr}")]
    Command {
        program: String,
        args: Vec<String>,
        status: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse output: {0}")]
    Parse(String),
    #[error("{0}")]
    Validation(String),
}

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Runs `program` with a fixed argv. There is deliberately no variant
    /// taking a shell string: that would be an interpolation surface.
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output, CoreError>;
}

pub struct SystemRunner;

#[async_trait]
impl CommandRunner for SystemRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output, CoreError> {
        tracing::info!(program, ?args, "running command");

        let output = tokio::process::Command::new(program)
            .args(args)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if output.status.success() {
            Ok(Output { stdout, stderr })
        } else {
            tracing::error!(program, ?args, status = ?output.status.code(), %stderr, "command failed");
            Err(CoreError::Command {
                program: program.to_string(),
                args: args.iter().map(|a| a.to_string()).collect(),
                status: output.status.code(),
                stderr: tail(&stderr),
            })
        }
    }
}

/// Returns at most the last `STDERR_TAIL_BYTES` of `s`, rounded to a char
/// boundary so lossy multi-byte output cannot panic here.
pub fn tail(s: &str) -> String {
    if s.len() <= STDERR_TAIL_BYTES {
        return s.to_string();
    }
    let start = s
        .char_indices()
        .rev()
        .map(|(i, _)| i)
        .find(|i| s.len() - i >= STDERR_TAIL_BYTES)
        .unwrap_or(0);
    s[start..].to_string()
}

enum Reply {
    Ok(String),
    Fail(i32, String),
}

/// Test double: returns queued replies in order and records every call.
pub struct RecordingRunner {
    replies: Mutex<std::collections::VecDeque<Reply>>,
    calls: Mutex<Vec<(String, Vec<String>)>>,
}

impl RecordingRunner {
    pub fn new() -> Self {
        Self {
            replies: Mutex::new(std::collections::VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn push_ok(&self, stdout: &str) {
        self.replies
            .lock()
            .unwrap()
            .push_back(Reply::Ok(stdout.to_string()));
    }

    pub fn push_fail(&self, status: i32, stderr: &str) {
        self.replies
            .lock()
            .unwrap()
            .push_back(Reply::Fail(status, stderr.to_string()));
    }

    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl Default for RecordingRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandRunner for RecordingRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output, CoreError> {
        self.calls.lock().unwrap().push((
            program.to_string(),
            args.iter().map(|a| a.to_string()).collect(),
        ));

        match self.replies.lock().unwrap().pop_front() {
            Some(Reply::Ok(stdout)) => Ok(Output {
                stdout,
                stderr: String::new(),
            }),
            Some(Reply::Fail(status, stderr)) => Err(CoreError::Command {
                program: program.to_string(),
                args: args.iter().map(|a| a.to_string()).collect(),
                status: Some(status),
                stderr,
            }),
            None => panic!("RecordingRunner: unexpected call to `{program}` with {args:?}"),
        }
    }
}
