// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Candidate publication checks the installed launcher's retained protocol authority.
use super::*;
use crate::setup_storage::{compatibility::Compatibility, entrypoint};

impl SelectionStore {
    /// Check while the caller retains setup or run custody, excluding launcher installation.
    pub(in crate::setup_storage) fn require_launcher(
        &self,
        compatibility: &Compatibility,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            entrypoint::stable::require_compatible(&self.root, compatibility, protection, context)?;
            Ok(())
        })
    }
}
