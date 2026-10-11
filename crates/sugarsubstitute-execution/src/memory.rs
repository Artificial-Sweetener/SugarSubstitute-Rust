// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Return an admitted peak-byte reservation only after its final retained guard.

use crate::{admission::Core, scope::lock};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{MemoryRelease, ScopeId};

pub(crate) struct Release {
    pub(crate) core: Arc<Core>,
    pub(crate) scope_id: ScopeId,
}

impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        {
            let core = &self.core;
            let mut state = lock(&core.state);
            state.bytes -= bytes;
            if let Some(scope) = state.scopes.get_mut(&self.scope_id) {
                scope.bytes -= bytes;
            }
            Core::retire_scopes(&mut state);
            let wake = core.capacity_ready(&mut state);
            drop(state);
            core.deliver_capacity_wake(wake);
            core.changed();
        }
    }
}
