// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the table charge outside each actual shared native owner allocation.
use super::{RegistryIdentity, Shared};
use std::{ops::Deref, sync::Arc};
/// Free shared native payload/control storage before releasing its external table pin.
/// Every clone carries both owners. Fields drop in this order and neither a raw Arc
/// nor Weak escapes this owner. The displaced inner pin keeps total table size fixed.
#[derive(Clone)]
pub(super) struct SharedOwner {
    shared: Arc<Shared>,
    _table_identity: RegistryIdentity,
}
impl SharedOwner {
    /// Adopt the exact shared state and an already admitted original table lifetime.
    pub(super) fn new(shared: Shared, identity: RegistryIdentity) -> Self {
        Self { shared: Arc::new(shared), _table_identity: identity }
    }
}
impl Deref for SharedOwner {
    type Target = Shared;
    /// Borrow native state while retaining its actual allocation and original table.
    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}
