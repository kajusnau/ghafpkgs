// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

pub mod page;
pub mod shell;

pub use page::{Page, PageMessage};
pub use shell::{MAX_HEIGHT, MAX_WIDTH, Nav, back_button_id, frame};
