// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::homed::{
    AccountConfig, UsernameError, fido_token_present, parse_config, username_available,
    validate_real_name, validate_username,
};
use ghaf_setup_core::proc::RecordingRunner;

#[test]
fn reads_the_config_user_provision_nix_writes() {
    let config = parse_config(
        r#"{ "user_config": { "home_size": 20000, "uid": 1000, "fs_type": "btrfs",
             "login_shell": "/bin/zsh", "groups": "users,wheel", "fido_auth": true } }"#,
    )
    .unwrap();
    assert_eq!(config.home_size_mib, 20000);
    assert_eq!(config.uid, Some(1000));
    assert_eq!(config.fs_type, "btrfs");
    assert_eq!(config.shell, "/bin/zsh");
    assert_eq!(config.groups, "users,wheel");
    assert!(config.fido_auth);
}

#[test]
fn missing_keys_fall_back_to_the_shell_defaults() {
    let config = parse_config(r#"{ "ad_config": { "domains": {} } }"#).unwrap();
    assert_eq!(config.home_size_mib, AccountConfig::default().home_size_mib);
    assert_eq!(config.shell, "/run/current-system/sw/bin/bash");
    assert!(!config.fido_auth);
}

#[test]
fn usernames_follow_the_systemd_rules() {
    assert!(validate_username("alice").is_ok());
    assert!(validate_username("_svc-2").is_ok());
    assert_eq!(validate_username(""), Err(UsernameError::Empty));
    assert_eq!(validate_username("2alice"), Err(UsernameError::BadStart));
    assert_eq!(validate_username("Alice"), Err(UsernameError::BadChar('A')));
    assert_eq!(validate_username("a b"), Err(UsernameError::BadChar(' ')));
    assert_eq!(
        validate_username(&"a".repeat(33)),
        Err(UsernameError::TooLong)
    );
}

#[test]
fn real_names_allow_any_letters_but_not_passwd_separators() {
    assert!(validate_real_name("José O'Brien").is_ok());
    assert!(validate_real_name("   ").is_err());
    assert!(validate_real_name("a:b").is_err());
    assert!(validate_real_name("a\nb").is_err());
}

#[tokio::test]
async fn a_name_getent_does_not_know_is_available() {
    let runner = RecordingRunner::new();
    runner.push_fail(2, "");
    assert!(username_available(&runner, "alice").await.unwrap());
    assert_eq!(runner.calls()[0].1, ["passwd", "alice"]);
}

#[tokio::test]
async fn a_name_getent_knows_is_taken() {
    let runner = RecordingRunner::new();
    runner.push_ok("root:x:0:0::/root:/bin/sh\n");
    assert!(!username_available(&runner, "root").await.unwrap());
}

#[tokio::test]
async fn fido_is_offered_only_when_a_token_is_listed() {
    let runner = RecordingRunner::new();
    runner.push_ok("/dev/hidraw0: vendor=0x1050, product=0x0407 (Yubico YubiKey)\n");
    assert!(fido_token_present(&runner).await);

    let none = RecordingRunner::new();
    none.push_ok("");
    assert!(!fido_token_present(&none).await);

    let broken = RecordingRunner::new();
    broken.push_fail(1, "fido2-token: not found");
    assert!(!fido_token_present(&broken).await);
}
