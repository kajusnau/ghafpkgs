// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Ghaf setup logic. Deliberately free of any UI dependency so it can be
//! tested without a compositor, a TTY or a GPU.

pub mod disk;
pub mod install;
pub mod proc;
pub mod progress;

pub use proc::{CommandRunner, CoreError, Output, RecordingRunner, SystemRunner};
