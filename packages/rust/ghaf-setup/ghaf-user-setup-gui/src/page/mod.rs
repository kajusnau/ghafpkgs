// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! The user setup wizard's pages.

pub mod account;
pub mod running;

use ghaf_setup_ui::Page;
use indexmap::IndexMap;
use std::any::TypeId;

pub fn pages(fido_available: bool) -> IndexMap<TypeId, Box<dyn Page>> {
    let mut pages: IndexMap<TypeId, Box<dyn Page>> = IndexMap::new();
    pages.insert(
        TypeId::of::<account::Page>(),
        Box::new(account::Page::new(fido_available)),
    );
    pages.insert(
        TypeId::of::<running::Page>(),
        Box::new(running::Page::default()),
    );
    pages
}
