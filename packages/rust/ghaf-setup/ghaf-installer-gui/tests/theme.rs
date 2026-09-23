// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use cosmic::cosmic_theme::ThemeBuilder;
use cosmic::cosmic_theme::palette::Srgb;
use ghaf_installer_gui::theme::load;
use std::path::PathBuf;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ghaf-theme-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("cosmic-themes")).unwrap();
    dir
}

#[test]
fn the_installed_theme_file_is_built_with_its_overrides() {
    let dir = data_dir("dark");
    let yellow = Srgb::new(0.98, 0.74, 0.24);
    let builder = ThemeBuilder::dark().warning(yellow);
    std::fs::write(
        dir.join("cosmic-themes/ghaf-dark.ron"),
        ron::to_string(&builder).unwrap(),
    )
    .unwrap();

    let data_dirs = format!("/nonexistent:{}", dir.display());
    let theme = load(&data_dirs).expect("theme loads");
    let warning = theme.cosmic().warning_color();
    assert!((warning.red - yellow.red).abs() < 0.01);
    assert!((warning.green - yellow.green).abs() < 0.01);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_theme_leaves_the_default() {
    assert!(load("/nonexistent").is_none());
}

#[test]
fn an_unparsable_theme_leaves_the_default() {
    let dir = data_dir("broken");
    std::fs::write(dir.join("cosmic-themes/ghaf-dark.ron"), "(").unwrap();
    assert!(load(&dir.display().to_string()).is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
