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

/// Retain the actual namespace directory slot, canonical name and creation identity.
pub(crate) struct DiagnosticDirectory {
    pub(super) path: NativeName,
    slot: usize,
    identity: [u8; 24],
}
impl ProtectedStorageRoot {
    /// Publish through the exact retained namespace parent without reopening it for writes.
    /// # Errors
    /// Rejects foreign budgets, changed parent identity/path and native rename failures.
    pub(crate) fn rename_diagnostic_file(
        &self,
        directory: &DiagnosticDirectory,
        source: &Handle,
        target: &Path,
        replace: bool,
        phase: &mut native_phase::NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        let state = self.state.as_ref().ok_or_else(|| phase.invalid("diagnostic_rename_root"))?;
        if !matches!(state.budget, Budget::Diagnostic(_)) {
            return Err(phase.invalid("diagnostic_rename_origin"));
        }
        phase.check_budget(&state.budget)?;
        let parent = state
            .handles
            .get(directory.slot)
            .ok_or_else(|| phase.invalid("diagnostic_rename_pin"))?;
        phase.check_budget(&parent.budget())?;
        if target.parent() != Some(directory.path.path()?)
            || paths::file_identity(parent, phase)? != directory.identity
        {
            return Err(phase.invalid("diagnostic_rename_parent"));
        }
        paths::validate_object(parent, true, true, phase)?;
        security::verify(parent, state.sid.bytes()?, phase)?;
        let destination = phase.name_destination()?;
        let actual = paths::final_path(parent, phase, destination)?;
        if actual.path()? != directory.path.path()? {
            return Err(phase.invalid("diagnostic_rename_parent_path"));
        }
        drop(actual);
        let destination = phase.name_destination()?;
        let actual_source = paths::final_path(source, phase, destination)?;
        if actual_source.path()?.parent() != Some(directory.path.path()?) {
            return Err(phase.invalid("diagnostic_rename_source_parent"));
        }
        drop(actual_source);
        record_file::native::rename(
            source,
            record_file::native::RenameTarget::SameDirectory(target),
            replace,
            phase,
        )
    }

    /// Revalidate the copied root against the same actual retained namespace pin.
    pub(crate) fn validate_diagnostic_namespace(
        &mut self,
        source: &crate::coordinator_entry::read_namespace::NamespaceReadLoan,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let state = self.state_mut()?;
        let original = match &state.budget {
            Budget::Diagnostic(original) => original.clone(),
            _ => return Err(state.budget.rejection("diagnostic_namespace_origin")),
        };
        let resolved = source.validate_writer(&original, context)?;
        bind_context(state, context, false)?;
        let (path, identity, index) = state
            .root
            .as_ref()
            .cloned()
            .ok_or_else(|| state.budget.rejection("diagnostic_namespace_root"))?;
        let (directory, _, directory_identity) = resolved.diagnostic_directory();
        let State { budget, causes, sid, handles, .. } = state;
        let pin = handles.get(index).ok_or_else(|| budget.rejection("diagnostic_namespace_pin"))?;
        with_phase(budget, causes, context, ScratchRole::Path, |phase| {
            if path.path()? != directory
                || identity != directory_identity
                || sid.bytes()? != resolved.account_sid.as_slice()
                || paths::file_identity(pin, phase)? != identity
            {
                return Err(phase.invalid("diagnostic_namespace_changed"));
            }
            paths::validate_object(pin, true, true, phase)?;
            security::verify(pin, sid.bytes()?, phase)?;
            phase.context(context)
        })?;
        source.validate_writer(&original, context)?;
        Ok(())
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
    ) -> Result<
        (DiagnosticDirectory, NativeSid, directories::DirectoryDisposition, [u8; 24]),
        ClaimError,
    > {
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
                        outputs: None,
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
        Ok((
            DiagnosticDirectory { path, slot: parent, identity: final_identity },
            sid.clone(),
            final_disposition,
            final_identity,
        ))
    }
}
