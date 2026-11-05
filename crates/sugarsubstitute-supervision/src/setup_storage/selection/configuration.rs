// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve unaccepted saved settings under selection custody without granting launch authority.
use super::*;
use crate::setup_storage::{entrypoint::stable, existing::ExistingInstallation};

impl SelectionStore {
    /// Prefer the exact explicit prepared choice; otherwise recover agreeing protected settings.
    /// A stable shell entry identifies the supported entry format, never a healthy application.
    /// Quarantine remains unchanged. Unknown/edited entries and conflicting settings stay errors.
    pub(in crate::setup_storage) fn unaccepted_configuration(
        &self,
        context: &JobContext,
    ) -> Result<Option<ExistingInstallation>, SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            let snapshot = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if snapshot.accepted().is_some() {
                return Ok(None);
            }
            if let Some(Candidate::Prepared { generation }) = snapshot.candidate() {
                let path = self._installation.path().join("releases").join(generation.id.as_str());
                protection.directory(&path, Mode::Existing, context)?;
                let directory = Directory::open(&path, false)?;
                if let Some(authorization) = completion::read(&directory, protection)? {
                    authorization.require(&self._installation, &directory, &snapshot)?;
                    let present = stable::validate_entry(
                        &self._installation,
                        &self.root,
                        protection,
                        context,
                    )?;
                    let existing = ExistingInstallation::from_prepared(
                        &self._installation,
                        directory,
                        generation,
                        present,
                        context,
                    )?;
                    check(context)?;
                    return Ok(Some(existing));
                }
            }
            let Some(present) =
                stable::owned_entry(&self._installation, &self.root, protection, context)?
            else {
                return Ok(None);
            };
            let existing =
                ExistingInstallation::recover_unaccepted(&self._installation, present, context)?;
            check(context)?;
            Ok(existing)
        })
    }
}
