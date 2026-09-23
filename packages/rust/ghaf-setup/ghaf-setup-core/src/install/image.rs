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

/// Extracts a fraction from a bmaptool progress line, e.g. "42% copied".
pub fn parse_bmaptool_progress(line: &str) -> Option<f32> {
    let idx = line.find('%')?;
    let digits: String = line[..idx]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    digits
        .parse::<f32>()
        .ok()
        .map(|p| (p / 100.0).clamp(0.0, 1.0))
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

    let result = runner
        .run("bmaptool", &["copy", "--bmap", &bmap, "--", &image, device])
        .await;

    match result {
        Ok(output) => {
            for line in output.stderr.lines() {
                if let Some(fraction) = parse_bmaptool_progress(line) {
                    let _ = progress.send(ProgressEvent::progress(Phase::WriteImage, fraction));
                }
            }
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
