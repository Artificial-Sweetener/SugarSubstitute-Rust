// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep diagnostic child evidence bounded and separate from native authentication.
use super::{Result, options::Role};
use serde::{Deserialize, Serialize};

/// Exact evidence emitted only after a normal child's native loop and workers retire.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChildReceipt {
    pub(crate) schema_version: u32,
    pub(crate) role: String,
    pub(crate) correlation: u64,
    pub(crate) original_deadline_qpc: u64,
    pub(crate) qpc_frequency: u64,
    pub(crate) original_execution_deadline_nanoseconds: u64,
    pub(crate) namespace: Option<String>,
    pub(crate) epoch: Option<String>,
    pub(crate) actual_failure_operation: Option<String>,
    pub(crate) backend_acquired: bool,
    pub(crate) backend_released: bool,
    pub(crate) claims_closed: bool,
    pub(crate) inheritance_cleared: bool,
    pub(crate) ready_signalled: bool,
    pub(crate) response_signalled: bool,
    pub(crate) release_observed: bool,
    pub(crate) async_finished: bool,
    pub(crate) cpu_finished: bool,
    pub(crate) native_finished: bool,
    pub(crate) reserved_bytes_after_teardown: u64,
    pub(crate) native_teardown_finished: bool,
}
impl ChildReceipt {
    /// Validate diagnostics after separately observing the exact contained process exit.
    /// This is never a native connection/authentication proof.
    pub(super) fn validate(
        &self,
        role: Role,
        correlation: u64,
        qpc: u64,
        frequency: u64,
    ) -> Result<()> {
        if self.schema_version != 1
            || self.role != role.wire()
            || self.correlation != correlation
            || self.original_deadline_qpc != qpc
            || self.qpc_frequency != frequency
            || self.original_execution_deadline_nanoseconds == 0
            || self.actual_failure_operation.as_deref() != role.expected_failure()
            || !(self.claims_closed
                && self.inheritance_cleared
                && self.ready_signalled
                && self.response_signalled
                && self.release_observed
                && self.async_finished
                && self.cpu_finished
                && self.native_finished
                && self.native_teardown_finished)
            || self.reserved_bytes_after_teardown != 0
        {
            return Err("Claim child receipt lacks exact native/operation/teardown evidence".into());
        }
        if role == Role::BackendSuccess && !(self.backend_acquired && self.backend_released) {
            return Err("Backend successor lacks actual acquisition and checked release".into());
        }
        if role != Role::BackendSuccess && (self.backend_acquired || self.backend_released) {
            return Err("Denied child reported a successful backend acquisition".into());
        }
        let has_owner =
            matches!(role, Role::BackendContender | Role::BackendSuccess | Role::ActiveHolder);
        if self.namespace.is_some() != has_owner || self.epoch.is_some() != has_owner {
            return Err("Claim child owner evidence differs from its exact role".into());
        }
        if let Some(namespace) = &self.namespace {
            canonical_hex(namespace, 64)?;
        }
        if let Some(epoch) = &self.epoch {
            canonical_hex(epoch, 32)?;
            if epoch.bytes().all(|v| v == b'0') {
                return Err("Claim child epoch is reserved".into());
            }
        }
        Ok(())
    }
}
fn canonical_hex(value: &str, count: usize) -> Result<()> {
    if value.len() != count || !value.bytes().all(|v| matches!(v,b'0'..=b'9'|b'a'..=b'f')) {
        return Err("Claim child digest evidence has an invalid shape".into());
    }
    Ok(())
}
/// Keep the normal exit and receipt together after actual child lifetime teardown.
pub(crate) struct ChildResult {
    pub(crate) exit_code: i32,
    pub(crate) path: std::path::PathBuf,
    pub(crate) receipt: serde_json::Value,
}

#[cfg(test)]
mod tests;
