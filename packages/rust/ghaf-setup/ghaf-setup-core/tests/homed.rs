// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::homed::{
    AccountConfig, AccountRequest, UsernameError, create_account, fido_token_present, parse_config,
    username_available, validate_real_name, validate_username,
};
use ghaf_setup_core::proc::RecordingRunner;
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use tokio::sync::mpsc::unbounded_channel;

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

fn request() -> AccountRequest {
    AccountRequest {
        username: "alice".into(),
        real_name: "Alice Liddell".into(),
        password: "correct horse".into(),
        fido: false,
        pin: None,
    }
}

#[tokio::test]
async fn creates_the_account_with_the_shells_homectl_arguments() {
    let runner = RecordingRunner::new();
    runner.push_ok("fhkbl-rtgvb-ltnri-kfbdv-ejirn-dcvgi-knlrv-vdlhb\n");
    let config = AccountConfig {
        uid: Some(1000),
        ..AccountConfig::default()
    };
    let (tx, _rx) = unbounded_channel();

    let key = create_account(&runner, &config, &request(), &tx)
        .await
        .unwrap();

    assert_eq!(
        key.unwrap().0,
        "fhkbl-rtgvb-ltnri-kfbdv-ejirn-dcvgi-knlrv-vdlhb"
    );
    let (program, args) = runner.calls().remove(0);
    assert_eq!(program, "homectl");
    assert_eq!(
        args,
        [
            "--no-ask-password",
            "create",
            "alice",
            "--real-name=Alice Liddell",
            "--skel=/etc/skel",
            "--storage=luks",
            "--luks-pbkdf-type=argon2id",
            "--fs-type=ext4",
            "--disk-size=10000M",
            "--drop-caches=true",
            "--nosuid=true",
            "--noexec=true",
            "--nodev=true",
            "--member-of=users",
            "--shell=/run/current-system/sw/bin/bash",
            "--enforce-password-policy=true",
            "--recovery-key=true",
            "--uid=1000",
        ]
    );
}

#[tokio::test]
async fn the_recovery_key_is_the_last_line_of_stdout() {
    let runner = RecordingRunner::new();
    runner.push_ok("Please note the recovery key:\nfhkbl-rtgvb-ltnri\n\n");
    let (tx, _rx) = unbounded_channel();

    let key = create_account(&runner, &AccountConfig::default(), &request(), &tx)
        .await
        .unwrap();

    assert_eq!(key.unwrap().0, "fhkbl-rtgvb-ltnri");
}

#[tokio::test]
async fn the_password_goes_through_the_environment_only() {
    let runner = RecordingRunner::new();
    runner.push_ok("key\n");
    let (tx, _rx) = unbounded_channel();

    create_account(&runner, &AccountConfig::default(), &request(), &tx)
        .await
        .unwrap();

    assert!(
        !runner.calls()[0]
            .1
            .iter()
            .any(|a| a.contains("correct horse"))
    );
    assert_eq!(
        runner.envs()[0],
        [("NEWPASSWORD".to_string(), "correct horse".to_string())]
    );
}

#[tokio::test]
async fn fido_adds_the_device_and_passes_the_pin() {
    let runner = RecordingRunner::new();
    runner.push_ok("key\n");
    let (tx, _rx) = unbounded_channel();
    let request = AccountRequest {
        fido: true,
        pin: Some("1234".into()),
        ..request()
    };

    create_account(&runner, &AccountConfig::default(), &request, &tx)
        .await
        .unwrap();

    assert!(
        runner.calls()[0]
            .1
            .contains(&"--fido2-device=auto".to_string())
    );
    assert!(runner.envs()[0].contains(&("PIN".to_string(), "1234".to_string())));
}

#[tokio::test]
async fn a_failed_create_is_reported_as_retryable() {
    let runner = RecordingRunner::new();
    runner.push_fail(1, "Password too weak: it is based on a dictionary word");
    let (tx, mut rx) = unbounded_channel();

    let result = create_account(&runner, &AccountConfig::default(), &request(), &tx).await;

    assert!(result.is_err());
    let mut failed = None;
    while let Ok(event) = rx.try_recv() {
        if let ProgressEvent::Failed {
            phase,
            message,
            recoverable,
        } = event
        {
            failed = Some((phase, recoverable, message));
        }
    }
    let (phase, recoverable, message) = failed.unwrap();
    assert_eq!(phase, Phase::CreateAccount);
    assert!(recoverable, "homectl leaves nothing behind on failure");
    assert!(message.contains("too weak"));
}

#[test]
fn a_request_never_prints_its_secrets() {
    let request = AccountRequest {
        pin: Some("1234".into()),
        ..request()
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("correct horse") && !debug.contains("1234"));
}

#[tokio::test]
async fn an_invalid_username_is_refused_before_homectl_runs() {
    let runner = RecordingRunner::new();
    let (tx, _rx) = unbounded_channel();
    let request = AccountRequest {
        username: "Bad Name".into(),
        ..request()
    };

    assert!(
        create_account(&runner, &AccountConfig::default(), &request, &tx)
            .await
            .is_err()
    );
    assert!(runner.calls().is_empty());
}
