// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The local login account: its settings, input rules and creation with
//! systemd-homed. Mirrors `user-provision.sh`'s local-user flow.

use crate::proc::{CommandRunner, CoreError};
use crate::progress::{Phase, ProgressEvent, ProgressSender};
use serde::Deserialize;
use std::path::Path;

pub const CONFIG_PATH: &str = "/etc/ghaf/provisioning.json";

/// systemd's limit on user names.
const MAX_USERNAME: usize = 32;

/// The account settings the image fixes; the user only picks the name,
/// real name and password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub home_size_mib: u64,
    pub uid: Option<u32>,
    pub fs_type: String,
    pub shell: String,
    pub groups: String,
    pub fido_auth: bool,
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            home_size_mib: 10000,
            uid: None,
            fs_type: "ext4".into(),
            shell: "/run/current-system/sw/bin/bash".into(),
            groups: "users".into(),
            fido_auth: false,
        }
    }
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    user_config: UserConfig,
}

#[derive(Deserialize, Default)]
struct UserConfig {
    home_size: Option<u64>,
    uid: Option<u32>,
    fs_type: Option<String>,
    login_shell: Option<String>,
    groups: Option<String>,
    fido_auth: Option<bool>,
}

pub fn parse_config(json: &str) -> Result<AccountConfig, CoreError> {
    let file: File =
        serde_json::from_str(json).map_err(|e| CoreError::Parse(format!("{CONFIG_PATH}: {e}")))?;
    let c = file.user_config;
    let d = AccountConfig::default();
    Ok(AccountConfig {
        home_size_mib: c.home_size.unwrap_or(d.home_size_mib),
        uid: c.uid,
        fs_type: c.fs_type.unwrap_or(d.fs_type),
        shell: c.login_shell.unwrap_or(d.shell),
        groups: c.groups.unwrap_or(d.groups),
        fido_auth: c.fido_auth.unwrap_or(d.fido_auth),
    })
}

/// Like the shell script, a missing or unreadable file means the defaults.
pub fn load_config(path: &Path) -> AccountConfig {
    match std::fs::read_to_string(path) {
        Ok(json) => parse_config(&json).unwrap_or_else(|error| {
            tracing::warn!(%error, "using default account settings");
            AccountConfig::default()
        }),
        Err(_) => AccountConfig::default(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameError {
    Empty,
    TooLong,
    BadStart,
    BadChar(char),
}

impl std::fmt::Display for UsernameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Enter a username."),
            Self::TooLong => write!(f, "Use at most {MAX_USERNAME} characters."),
            Self::BadStart => write!(f, "Start with a lowercase letter or _."),
            Self::BadChar(c) => write!(
                f,
                "\"{c}\" is not allowed: use lowercase letters, digits, _ and -."
            ),
        }
    }
}

/// Checked as the user types rather than silently rewritten, unlike the
/// shell, which quietly lowercased and stripped characters.
pub fn validate_username(name: &str) -> Result<(), UsernameError> {
    let first = name.chars().next().ok_or(UsernameError::Empty)?;
    if name.chars().count() > MAX_USERNAME {
        return Err(UsernameError::TooLong);
    }
    match name
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-'))
    {
        Some(c) => Err(UsernameError::BadChar(c)),
        None => {
            if !(first.is_ascii_lowercase() || first == '_') {
                Err(UsernameError::BadStart)
            } else {
                Ok(())
            }
        }
    }
}

/// Anything printable except what would break a passwd entry. The shell
/// allowed only ASCII letters and spaces, which rejects most real names.
pub fn validate_real_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Enter your name.".into());
    }
    if name.chars().any(|c| c == ':' || c.is_control()) {
        return Err("A name cannot contain \":\" or line breaks.".into());
    }
    Ok(())
}

/// `getent` exits 2 for an unknown name; any other failure is an error.
pub async fn username_available(runner: &dyn CommandRunner, name: &str) -> Result<bool, CoreError> {
    match runner.run("getent", &["passwd", name]).await {
        Ok(_) => Ok(false),
        Err(CoreError::Command {
            status: Some(2), ..
        }) => Ok(true),
        Err(error) => Err(error),
    }
}

/// Whether a FIDO2 token is plugged in. A failure answers "no": offering a
/// token that cannot be used is worse than not offering one.
pub async fn fido_token_present(runner: &dyn CommandRunner) -> bool {
    runner
        .run("fido2-token", &["-L"])
        .await
        .is_ok_and(|output| !output.stdout.trim().is_empty())
}

/// What the user entered. Held only until `homectl` has run.
#[derive(Clone)]
pub struct AccountRequest {
    pub username: String,
    pub real_name: String,
    pub password: String,
    pub fido: bool,
    /// The FIDO2 token's PIN, when it has one.
    pub pin: Option<String>,
}

impl std::fmt::Debug for AccountRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountRequest")
            .field("username", &self.username)
            .field("real_name", &self.real_name)
            .field("fido", &self.fido)
            .finish_non_exhaustive()
    }
}

/// The key that unlocks the home area without the password. Shown once.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKey(pub String);

impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKey(..)")
    }
}

pub async fn create_account(
    runner: &dyn CommandRunner,
    config: &AccountConfig,
    request: &AccountRequest,
    progress: &ProgressSender,
) -> Result<Option<RecoveryKey>, CoreError> {
    validate_username(&request.username).map_err(|e| CoreError::Validation(e.to_string()))?;
    validate_real_name(&request.real_name).map_err(CoreError::Validation)?;

    let _ = progress.send(ProgressEvent::PhaseStarted(Phase::CreateAccount));

    let owned = [
        format!("--real-name={}", request.real_name),
        format!("--fs-type={}", config.fs_type),
        format!("--disk-size={}M", config.home_size_mib),
        format!("--member-of={}", config.groups),
        format!("--shell={}", config.shell),
    ];
    let uid = config.uid.map(|uid| format!("--uid={uid}"));
    // A prompt nobody can see would hang the run: fail with stderr instead.
    let mut args = vec![
        "--no-ask-password",
        "create",
        &request.username,
        &owned[0],
        "--skel=/etc/skel",
        "--storage=luks",
        "--luks-pbkdf-type=argon2id",
        &owned[1],
        &owned[2],
        "--drop-caches=true",
        "--nosuid=true",
        "--noexec=true",
        "--nodev=true",
        &owned[3],
        &owned[4],
        "--enforce-password-policy=true",
        "--recovery-key=true",
    ];
    args.extend(uid.as_deref());
    if request.fido {
        args.push("--fido2-device=auto");
    }

    let mut env = vec![("NEWPASSWORD", request.password.as_str())];
    env.extend(request.pin.as_deref().map(|pin| ("PIN", pin)));

    match runner.run_env("homectl", &args, &env).await {
        Ok(output) => {
            let _ = progress.send(ProgressEvent::PhaseFinished(Phase::CreateAccount));
            let key = output
                .stdout
                .lines()
                .map(str::trim)
                .rfind(|line| !line.is_empty())
                .unwrap_or_default();
            Ok((!key.is_empty()).then(|| RecoveryKey(key.to_string())))
        }
        Err(error) => {
            // homectl removes a half-made home area itself, so the user can
            // correct the input and try again.
            let message = match &error {
                CoreError::Command { stderr, .. } if !stderr.trim().is_empty() => {
                    stderr.trim().to_string()
                }
                other => other.to_string(),
            };
            let _ = progress.send(ProgressEvent::Failed {
                phase: Phase::CreateAccount,
                message,
                recoverable: true,
            });
            Err(error)
        }
    }
}
