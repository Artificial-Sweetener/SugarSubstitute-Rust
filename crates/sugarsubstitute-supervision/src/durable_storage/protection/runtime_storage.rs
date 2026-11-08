// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve installation-local account runtime storage within admitted durable operations.
use super::*;
impl Protection {
    /// Read the actual primary account's earlier runtime location without creating it.
    /// The caller must verify each existing feature namespace before importing any data.
    pub(crate) fn historical_runtime(
        &mut self,
        context: &JobContext,
    ) -> io::Result<std::path::PathBuf> {
        check(context)?;
        let path =
            account::runtime_base::historical(&mut self.handles, self.budget.clone(), &self.sid)
                .map_err(io::Error::other)?;
        check(context)?;
        Ok(path.join("SugarSubstitute").join("runtime"))
    }
    /// Resolve a feature-owned runtime child `<base>\runtime\<leaf>` through observed native names.
    ///
    /// `base` is an installation root named by the configuration layout owner; records never
    /// reach account profile folders. Creation also creates a missing base. Existing-only access
    /// neither creates missing directories nor repairs foreign protection.
    pub(crate) fn runtime_storage(
        &mut self,
        base: &std::path::Path,
        leaf: &'static str,
        create: bool,
        context: &JobContext,
    ) -> io::Result<std::path::PathBuf> {
        check(context)?;
        self.require_primary_account(context)?;
        if create {
            match crate::durable_storage::files::Directory::open(base, false) {
                Ok(_) => {}
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    let parent = base
                        .parent()
                        .ok_or_else(|| super::super::invalid("Runtime base parent missing"))?;
                    let _parent = crate::durable_storage::files::Directory::open(parent, true)?;
                    self.directory(
                        base,
                        crate::durable_storage::protected::directories::Mode::CreateNew,
                        context,
                    )?;
                }
                Err(cause) => return Err(cause),
            }
        }
        let directory = account::runtime_base::pin(base, &mut self.handles, self.budget.clone())
            .map_err(io::Error::other)?;
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
    /// Require the same actual primary account that owns this protection, without consulting
    /// environment overrides; runtime records are protected for that account alone.
    fn require_primary_account(&mut self, context: &JobContext) -> io::Result<()> {
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
        Ok(())
    }
}
#[cfg(test)]
mod tests;
