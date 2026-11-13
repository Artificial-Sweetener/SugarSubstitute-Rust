// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one authenticated custody helper in the existing three-helper partition.
use super::*;
use sugarsubstitute_contracts::{backend_custody::BackendCustodyKey, execution::ScopeSpec};
impl WindowsProcessRegistry {
    /// Observe the exact original helper's live launch without sampling a clock or retiring it.
    /// # Errors
    /// Preserves the first owned registry failure and rejects foreign bindings or an actual root exit.
    pub fn backend_custody_launch_pending(
        &self,
        process: ProcessKey,
        key: BackendCustodyKey,
    ) -> Result<bool, RegistryError> {
        let index = self
            .index(process)
            .map_err(|cause| RegistryError::new("custody_live_record", cause))?;
        let record = self.records[index]
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("custody_live_record"))?;
        if record.ticket.binding != FamilyBinding::BackendCustody(key) {
            return Err(RegistryError::invalid("custody_live_binding"));
        }
        if let Some(cause) = &record.error {
            return Err(cause.clone());
        }
        helper_root_alive(record.root_exit)?;
        Ok(record.started)
    }

    /// Preserve the original input on rejection and retain the independent helper family.
    /// The actual Windows creation uses helper-only breakaway from enclosing Jobs;
    /// forbidden breakaway fails contained before backend custody can begin.
    /// # Errors
    /// Preserves native watch/admission causes and rejects foreign session pins or quota.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_backend_custody_launch(
        &mut self,
        execution: &ExecutionClient,
        creation_scope: &Scope,
        native_scope_spec: ScopeSpec,
        creation: JobSpec,
        root_watch: JobSpec,
        key: BackendCustodyKey,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<LaunchTicket, RegistryAdmissionError> {
        self.submit_owned(
            execution,
            creation_scope,
            native_scope_spec,
            creation,
            root_watch,
            FamilyBinding::BackendCustody(key),
            input,
        )
    }
}

/// Classify only an already-correlated original root observation, without retirement effects.
pub(super) fn helper_root_alive(exit: Option<ProcessExit>) -> Result<(), RegistryError> {
    if let Some(exit) = exit {
        return Err(RegistryError::new(
            "custody_helper_exited",
            std::io::Error::other(format!(
                "Actual owned custody helper exited with code {}",
                exit.exit_code
            )),
        ));
    }
    Ok(())
}
