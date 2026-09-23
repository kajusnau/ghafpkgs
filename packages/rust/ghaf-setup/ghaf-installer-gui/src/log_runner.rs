// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Copies every command and its output into the running page's log.

use async_trait::async_trait;
use ghaf_setup_core::proc::{CommandRunner, CoreError, Output};
use ghaf_setup_core::progress::{ProgressEvent, ProgressSender};

/// Output lines kept per command: enough to follow along, not a dump of
/// lsblk's JSON.
pub const MAX_OUTPUT_LINES: usize = 20;

pub struct LoggingRunner<R> {
    inner: R,
    progress: ProgressSender,
}

impl<R> LoggingRunner<R> {
    pub fn new(inner: R, progress: ProgressSender) -> Self {
        Self { inner, progress }
    }

    fn log(&self, line: String) {
        let _ = self.progress.send(ProgressEvent::Log(line));
    }

    fn log_output(&self, text: &str) {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let skipped = lines.len().saturating_sub(MAX_OUTPUT_LINES);
        if skipped > 0 {
            self.log(format!("  … {skipped} more lines"));
        }
        for line in &lines[skipped..] {
            self.log(format!("  {line}"));
        }
    }
}

#[async_trait]
impl<R: CommandRunner> CommandRunner for LoggingRunner<R> {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output, CoreError> {
        self.log(format!("$ {program} {}", args.join(" ")));
        let result = self.inner.run(program, args).await;
        match &result {
            Ok(output) => {
                self.log_output(&output.stdout);
                self.log_output(&output.stderr);
            }
            Err(error) => self.log(format!("  {error}")),
        }
        result
    }
}
