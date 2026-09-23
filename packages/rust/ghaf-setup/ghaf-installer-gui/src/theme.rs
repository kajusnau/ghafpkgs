// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The Ghaf COSMIC theme, built in-process: a kiosk session has no
//! cosmic-settings-daemon to derive the full theme from the installed files.

use cosmic::cosmic_theme::ThemeBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Finds the first file at `relative` under the colon-separated `data_dirs`.
fn find(data_dirs: &str, relative: &str) -> Option<PathBuf> {
    data_dirs
        .split(':')
        .map(|dir| Path::new(dir).join(relative))
        .find(|path| path.is_file())
}

/// Loads `ghaf-dark.ron` or `ghaf-light.ron` per the installed COSMIC mode.
pub fn load(data_dirs: &str) -> Option<cosmic::Theme> {
    let dark = find(data_dirs, "cosmic/com.system76.CosmicTheme.Mode/v1/is_dark")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_none_or(|value| value.trim() != "false");
    let name = if dark { "ghaf-dark" } else { "ghaf-light" };

    let source =
        std::fs::read_to_string(find(data_dirs, &format!("cosmic-themes/{name}.ron"))?).ok()?;
    match ron::from_str::<ThemeBuilder>(&source) {
        Ok(builder) => Some(cosmic::Theme::custom(Arc::new(builder.build()))),
        Err(error) => {
            tracing::warn!(%error, "cannot parse the {name} theme");
            None
        }
    }
}
