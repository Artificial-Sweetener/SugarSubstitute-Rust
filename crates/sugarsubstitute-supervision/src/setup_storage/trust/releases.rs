// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain publication floors under the same native exclusion as signing authority.
use super::*;
use sugarsubstitute_update::ReleaseHistory;

pub(super) const RECORD: &str = "releases";

impl TrustStore {
    /// Missing history requires verified recovery; a valid key checkpoint cannot replace it.
    pub(super) fn read_releases(
        &self,
        protection: &Protection,
    ) -> Result<ReleaseHistory, TrustFailure> {
        let mut file = self.root.read(RECORD)?;
        protection.verify_file(&file)?;
        let bytes = files::read_bounded(&mut file, envelope::Kind::Releases.limit())?;
        let payload = envelope::decode(
            envelope::Kind::Releases,
            &self.installation,
            &self.repository,
            &bytes,
        )?;
        Ok(ReleaseHistory::decode(payload)?)
    }

    /// Commit changed floors before shell publication. `previous` was read and verified
    /// under the caller's still-held trust writer. Identical history remains durable in place.
    /// Failure leaves the previous selection unchanged;
    /// a later activation failure retains the conservative floor and never fabricates health.
    pub(super) fn write_releases(
        &self,
        previous: &ReleaseHistory,
        history: &ReleaseHistory,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let payload = history.encode()?;
        if payload == previous.encode()? {
            check(context)?;
            return Ok(());
        }
        let bytes = envelope::encode(
            envelope::Kind::Releases,
            &self.installation,
            &self.repository,
            &payload,
        )?;
        let staged = staging::unique_path(&self.root, "releases-pending")?;
        let mut file = protection.file(&staged, context)?;
        check(context)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        staging::replace_record(&staged, &self.root.path().join(RECORD))?;
        check(context)?;
        Ok(())
    }
}
