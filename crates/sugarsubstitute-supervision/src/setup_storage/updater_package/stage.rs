// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stage a complete updater generation before publishing preparation; application selection stays separate.
use super::*;
use crate::durable_storage::{files::Directory, protected::directories::Mode, protection, staging};
use std::{fs::File, io};
use sugarsubstitute_contracts::installation::Generation;
mod trial;
pub use trial::{RejectedUpdaterProbe, UnstartedUpdaterTrial, UpdaterProbeTransfer};

/// Worker admission beside retained package/trust custody, including destination verification
/// and overlapping native publication operations. This does not fund another download.
pub const UPDATER_STAGING_RESERVATION_BYTES: u64 = crate::setup_storage::RESERVATION_BYTES
    + 2 * crate::setup_storage::TRUST_OPERATION_RESERVATION_BYTES;

/// Complete pinned engine generation, retained source evidence and live preparation exclusion.
/// This is preparation evidence only. No process health or activation is implied by construction.
pub struct PreparedUpdater {
    source: UpdaterPackage,
    generation: Generation,
    payload: Payload,
    _preparation: File,
}
impl PreparedUpdater {
    /// Exact durable generation that completed verification, independent of app generations.
    pub fn generation(&self) -> &Generation {
        &self.generation
    }
    /// Source inventory identity retained through this prepared generation's lifetime.
    pub fn digest(&self) -> &str {
        self.source.digest()
    }
}

impl UpdaterPackage {
    /// Resume or stage a complete engine generation and publish its prepared state atomically.
    /// Run on an admitted worker with `UPDATER_STAGING_RESERVATION_BYTES` beside retained package
    /// custody. `now` supplies Unix UTC seconds; current trust is checked before and after copy.
    /// Engine preparation owns a separate OS lock and never takes application run or setup locks.
    /// Fresh exclusion quarantines an abandoned trial before reserving the authenticated offer.
    /// # Errors
    /// Rejects live engine preparation/trials, changed authority, unsafe/altered destinations,
    /// cancellation and disk failure. Previous selections, receipts and partials are preserved.
    pub fn stage(
        self,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<PreparedUpdater, TrustFailure> {
        check(context)?;
        let preparation = self.trust.claim_updater_preparation(context)?;
        let authorization = self.offer.authorization(&self.trust);
        let bytes = self.download.identity();
        let (staged, generation) =
            authorization.authenticate_updater(bytes, &mut now, context, |offer, _| {
                self.trust
                    .reserve_updater(
                        authorization.request,
                        offer,
                        self.digest(),
                        &preparation,
                        context,
                    )
                    .map_err(io::Error::other)
            })?;
        let payload = self.stage_payload(&generation, context)?;
        staged.commit(now, context, |_, recheck| {
            recheck()?;
            self.trust.complete_updater(&generation, context).map_err(io::Error::other)
        })?;
        Ok(PreparedUpdater { source: self, generation, payload, _preparation: preparation })
    }

    /// Publish a complete directory through checked native identity; existing exact bytes resume.
    fn stage_payload(&self, generation: &Generation, context: &JobContext) -> io::Result<Payload> {
        let destination = self.trust.updater_path(generation);
        match Directory::open(&destination, false) {
            Ok(_directory) => return self.open_staged(&destination, context),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
        let parent = Directory::open(
            destination.parent().ok_or_else(|| invalid("Updater parent absent"))?,
            false,
        )?;
        let staged = staging::unique_path(&parent, ".engine-stage")?;
        protection::with(context, |protection| {
            protection.directory(&staged, Mode::CreateNew, context)?;
            protection.directory(&staged.join("app"), Mode::CreateNew, context)?;
            self.payload.copy("", &staged, context, |path| protection.file(path, context))?;
            self.payload.retain_manifest(&staged, |path| protection.file(path, context))
        })?;
        let identity = {
            let _verified = self.open_staged(&staged, context)?;
            Directory::open(&staged, false)?.identity()?
        };
        protection::with(context, |protection| {
            protection.publish(
                &staged,
                &destination,
                identity,
                protection::Publication::Directory,
                context,
            )
        })?;
        let payload = self.open_staged(&destination, context)?;
        if Directory::open(&destination, false)?.identity()? != identity {
            return Err(invalid("Published updater directory changed"));
        }
        Ok(payload)
    }

    /// Require the actual protected destination and every authenticated package byte on each retry.
    fn open_staged(&self, path: &Path, context: &JobContext) -> io::Result<Payload> {
        protection::with(context, |protection| {
            protection.directory(path, Mode::Existing, context)?;
            protection.directory(&path.join("app"), Mode::Existing, context)?;
            let payload = Payload::open(path, self.digest(), Component::Updater, context)?;
            payload.require_protected(protection)?;
            Ok(payload)
        })
    }
}
