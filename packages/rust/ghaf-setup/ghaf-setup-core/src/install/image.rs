// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Writing the Ghaf image to the target disk.
//!
//! bmaptool reads http(s) sources itself, so the netboot case needs no
//! separate download step and no temporary file large enough to hold the
//! image -- which the installer environment does not have.

use crate::disk::validate_path;
use crate::proc::{CommandRunner, CoreError};
use crate::progress::{Phase, ProgressEvent, ProgressSender};

const IMAGE_NAME: &str = "ghaf-image.raw.zst";
const BMAP_NAME: &str = "ghaf-image.bmap";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    LocalDir(String),
    Url(String),
}

impl ImageSource {
    /// Mirrors the shell installer's scheme test on IMG_PATH.
    pub fn parse(img_path: &str) -> Self {
        if img_path.starts_with("http://") || img_path.starts_with("https://") {
            ImageSource::Url(img_path.trim_end_matches('/').to_string())
        } else {
            ImageSource::LocalDir(img_path.trim_end_matches('/').to_string())
        }
    }

    fn base(&self) -> &str {
        match self {
            ImageSource::LocalDir(p) | ImageSource::Url(p) => p,
        }
    }

    fn image(&self) -> String {
        format!("{}/{}", self.base(), IMAGE_NAME)
    }

    fn bmap(&self) -> String {
        format!("{}/{}", self.base(), BMAP_NAME)
    }
}

/// Extracts a fraction from a psplash command bmaptool sends, e.g.
/// "PROGRESS 42".
pub fn parse_bmaptool_progress(line: &str) -> Option<f32> {
    let percent = line.trim().strip_prefix("PROGRESS ")?.parse::<f32>().ok()?;
    Some((percent / 100.0).clamp(0.0, 1.0))
}

/// A FIFO for bmaptool's `--psplash-pipe`: its only progress output that
/// does not need a terminal. Held open for reading and writing, since
/// bmaptool opens it non-blocking and skips an update when no reader is
/// there, and so the reader never sees end of file between updates.
struct ProgressPipe {
    path: std::path::PathBuf,
    reader: tokio::task::JoinHandle<()>,
}

impl ProgressPipe {
    fn open(progress: &ProgressSender) -> std::io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        use tokio::io::AsyncBufReadExt;

        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ghaf-installer-progress-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: c_path is a valid NUL-terminated string for the call.
        if unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let pipe = tokio::net::unix::pipe::OpenOptions::new()
            .read_write(true)
            .open_receiver(&path)?;

        let progress = progress.clone();
        let reader = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(fraction) = parse_bmaptool_progress(&line) {
                    let _ = progress.send(ProgressEvent::progress(Phase::WriteImage, fraction));
                }
            }
        });
        Ok(Self { path, reader })
    }
}

impl Drop for ProgressPipe {
    fn drop(&mut self) {
        self.reader.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn write_image(
    runner: &dyn CommandRunner,
    source: &ImageSource,
    device: &str,
    progress: &ProgressSender,
) -> Result<(), CoreError> {
    validate_path(device)?;
    let _ = progress.send(ProgressEvent::PhaseStarted(Phase::WriteImage));

    let bmap = source.bmap();
    let image = source.image();

    // Progress is decoration: install without it rather than fail.
    let pipe = ProgressPipe::open(progress)
        .inspect_err(|error| tracing::warn!(%error, "no progress from bmaptool"))
        .ok();
    let pipe_path = pipe.as_ref().map(|p| p.path.to_string_lossy().into_owned());

    let mut args = vec!["copy", "--bmap", &bmap];
    if let Some(path) = &pipe_path {
        args.extend(["--psplash-pipe", path]);
    }
    args.extend(["--", &image, device]);

    let result = runner.run("bmaptool", &args).await;
    drop(pipe);

    match result {
        Ok(_) => {
            let _ = progress.send(ProgressEvent::PhaseFinished(Phase::WriteImage));
            Ok(())
        }
        Err(error) => {
            // The disk has been written to by this point. There is no safe
            // retry: the UI must send the user to a terminal screen.
            let _ = progress.send(ProgressEvent::Failed {
                phase: Phase::WriteImage,
                message: error.to_string(),
                recoverable: false,
            });
            Err(error)
        }
    }
}
