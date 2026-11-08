// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve open occurrence identity while admitting fresh pages only through reconciliation.
use sugarsubstitute_contracts::desktop_session::{SessionError, wire::SessionFence};

/// One live runtime object keeps its durable occurrence through rename and Save As. A duplicate
/// supplies a new runtime and entry identity. A fresh webview establishes new runtime bindings
/// through materialization, rather than replacing an existing live occurrence implicitly.
pub(super) fn admit(previous: &SessionFence, next: &SessionFence) -> Result<(), SessionError> {
    for binding in &next.bindings {
        if previous.bindings.iter().any(|old| {
            match (old.entry_id == binding.entry_id, old.runtime_session == binding.runtime_session)
            {
                (true, true) => binding.generation < old.generation,
                (false, false) => false,
                _ => true,
            }
        }) {
            return Err(SessionError::Stale);
        }
    }
    Ok(())
}
