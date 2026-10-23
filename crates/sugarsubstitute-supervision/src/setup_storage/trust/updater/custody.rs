// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep updater exclusion attached to its native file object through every retained duplicate.
use super::*;

const RENDEZVOUS: &str = "updater-preparation-v1";

impl TrustStore {
    /// Bind a retained exclusive file to this installation's canonical updater rendezvous.
    /// This proves native custody, not fresh acquisition or process settlement.
    pub(super) fn verify_updater_preparation(
        &self,
        file: &File,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        protection.verify_exclusive_rendezvous(file, &self.root.path.join(RENDEZVOUS), context)?;
        Ok(())
    }

    /// Exclude other updater preparation until the last duplicate of this file object closes.
    /// The protected empty rendezvous also conflicts with historical shared byte-range locks.
    /// File existence, process IDs and elapsed time never authorize takeover. This owner acquires
    /// no application run or setup lease. Native process transfer must retain the same object.
    pub(in crate::setup_storage) fn claim_updater_preparation(
        &self,
        context: &JobContext,
    ) -> Result<File, TrustFailure> {
        self.protected(context, |protection| {
            let file =
                protection.exclusive_rendezvous(&self.root.path.join(RENDEZVOUS), context)?;
            check(context)?;
            Ok(file)
        })
    }
}

#[cfg(test)]
mod tests;
