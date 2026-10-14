// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the exact Diagnostic namespace through its closed original allocation.
use super::*;
use resolution::{ScratchRole, with_phase};
/// Encode the same authoritative namespace leaf without allocating native paths.
pub(crate) fn namespace_component(
    namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
) -> [u8; 64] {
    let mut hex = [0_u8; 64];
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in namespace.bytes().iter().enumerate() {
        hex[index * 2] = DIGITS[(byte >> 4) as usize];
        hex[index * 2 + 1] = DIGITS[(byte & 15) as usize];
    }
    hex
}

impl ProtectedStorageRoot {
    /// Revalidate the copied root against the same actual retained namespace pin.
    pub(crate) fn validate_diagnostic_namespace(
        &mut self,
        source: &account::ResolvedNamespace,
        pins: &[Handle],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let state = self.state_mut()?;
        bind_context(state, context, false)?;
        let (path, identity, _) = state
            .root
            .as_ref()
            .cloned()
            .ok_or_else(|| state.budget.rejection("diagnostic_namespace_root"))?;
        let (directory, slot, directory_identity) = source.diagnostic_directory();
        let pin =
            pins.get(slot).ok_or_else(|| state.budget.rejection("diagnostic_namespace_pin"))?;
        let State { budget, causes, sid, .. } = state;
        with_phase(budget, causes, context, ScratchRole::Path, |phase| {
            if path.path()? != directory
                || identity != directory_identity
                || sid.bytes()? != source.account_sid.as_slice()
                || paths::file_identity(pin, phase)? != identity
            {
                return Err(phase.invalid("diagnostic_namespace_changed"));
            }
            paths::validate_object(pin, true, true, phase)?;
            security::verify(pin, sid.bytes()?, phase)?;
            phase.context(context)
        })
    }

    /// Retire only retained outputs under the actual storage cleanup binding.
    /// Each fresh close checks that binding; accepted late returns are adopted
    /// passively. Errors preserve the complete root Box and outside owner loan.
    pub(crate) fn retire_diagnostic(
        &mut self,
        retirement: &mut crate::diagnostic_storage::DiagnosticNativeRetirement<'_>,
    ) -> Result<(), ClaimError> {
        if let Some(state) = self.state.as_mut() {
            if !matches!(state.budget, Budget::Diagnostic(_)) {
                return Err(state.budget.rejection("diagnostic_retirement_origin"));
            }
            for handle in state.handles.iter_mut().rev() {
                handle.close_diagnostic(retirement, &mut state.causes)?;
            }
            state.handles.clear();
        }
        self.state = None;
        self.allocation = None;
        Ok(())
    }
    /// Open the protected namespace without copied ordinary names or SID grants.
    /// # Errors
    /// Rejects ordinary origins, changed context, missing root and partial protection.
    pub(crate) fn diagnostic_directory(
        &mut self,
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
        context: &JobContext,
    ) -> Result<(NativeName, NativeSid, directories::DirectoryDisposition, [u8; 24]), ClaimError>
    {
        let state = self.state_mut()?;
        if !matches!(state.budget, Budget::Diagnostic(_)) {
            return Err(state.budget.rejection("diagnostic_directory_origin"));
        }
        bind_context(state, context, false)?;
        let (mut path, _, mut parent) = state
            .root
            .as_ref()
            .cloned()
            .ok_or_else(|| state.budget.rejection("unresolved_storage"))?;
        let mut hex = [0_u8; 64];
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in namespace.bytes().iter().enumerate() {
            hex[index * 2] = DIGITS[(byte >> 4) as usize];
            hex[index * 2 + 1] = DIGITS[(byte & 15) as usize];
        }
        let leaf = std::str::from_utf8(&hex)
            .map_err(|_| state.budget.rejection("diagnostic_namespace_hex"))?;
        let State { budget, handles, sid, causes, .. } = state;
        let mut final_disposition = directories::DirectoryDisposition::Existing;
        let mut final_identity = [0; 24];
        for child in ["diagnostics", "incident-v1", leaf] {
            let resolved = with_phase(budget, causes, context, ScratchRole::Path, |phase| {
                let destination = phase.name_destination()?;
                let target = destination.join(path.path()?, std::ffi::OsStr::new(child), phase)?;
                let index = directories::open_relative(
                    handles,
                    sid.bytes()?,
                    budget.clone(),
                    directories::RelativeDirectory {
                        path: target.path()?,
                        parent,
                        mode: directories::Mode::OpenOrCreate,
                        output: None,
                    },
                    context,
                    phase,
                )?;
                drop(target);
                let destination = phase.name_destination()?;
                Ok((paths::final_path(&handles[index.index], phase, destination)?, index))
            })?;
            path = resolved.0;
            parent = resolved.1.index;
            final_disposition = resolved.1.disposition;
            final_identity = resolved.1.identity;
        }
        Ok((path, sid.clone(), final_disposition, final_identity))
    }
}
