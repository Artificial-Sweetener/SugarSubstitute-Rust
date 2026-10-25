// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse native primary-account storage resolution within admitted durable operations.
use super::*;
impl Protection {
    /// Resolve a feature-owned runtime child through the account namespace's observed native names.
    /// Existing-only access neither creates missing directories nor repairs foreign protection.
    pub(crate) fn runtime_storage(
        &mut self,
        leaf: &'static str,
        create: bool,
        context: &JobContext,
    ) -> io::Result<std::path::PathBuf> {
        check(context)?;
        let directory = self.local_storage(context)?;
        let (directory, _, _) = account::namespace_directory::resolve(
            directory,
            account::namespace_directory::Namespace::RuntimeChild(leaf),
            create,
            &self.sid,
            self.budget.clone(),
            &mut self.handles,
        )
        .map_err(io::Error::other)?;
        check(context)?;
        Ok(directory)
    }
    /// Resolve the same actual primary account, without consulting environment path overrides.
    fn local_storage(&mut self, context: &JobContext) -> io::Result<std::path::PathBuf> {
        check(context)?;
        let mut phase = NativePhase::ordinary(&self.budget).map_err(io::Error::other)?;
        let account = account::resolve_current(
            &mut self.handles,
            self.budget.clone(),
            account::PrimaryAccountAccess::AccountStorage,
            &mut phase,
        )
        .map_err(io::Error::other)?;
        if account.sid.bytes().map_err(io::Error::other)? != self.sid {
            return Err(super::super::invalid("Primary diagnostic account changed"));
        }
        account::local_storage::resolve(
            self.handles[account.token_slot].raw(),
            &mut self.handles,
            self.budget.clone(),
        )
        .map_err(io::Error::other)
    }
}
#[cfg(test)]
mod tests;
