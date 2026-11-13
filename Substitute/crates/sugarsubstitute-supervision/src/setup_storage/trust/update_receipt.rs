// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep a prepared update startable offline through a protected local receipt (D1, D22).
//!
//! The receipt is written in the same trust-writer hold that re-verifies and publishes the
//! prepared selection record, so it only ever describes a release that current signed authority
//! admitted. Like `InstallationAuthorization`, it binds the native installation and generation
//! directories by identity, so a copied file never authorises another folder. Unlike that record
//! it is not a standing permission: [`TrustStore::admit_receipt`] re-judges it at every launch
//! against current trust, release history and any newer metadata.
//!
//! The record is forward-tolerant: readers ignore fields they do not know, and the schema version
//! changes only for an incompatible change, so an older launcher can still read a newer receipt.
use super::*;
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::installation::{Candidate, Generation, InstallationRecord};
use sugarsubstitute_update::PreparedRelease;

/// Receipt file inside the prepared generation directory.
pub(crate) const FILE: &str = "update-receipt.record";
/// Prefix of the stage a receipt is written to before it replaces [FILE]. Dot-led, so the
/// generation's retirement inventory can own a stage an interruption left behind.
pub(crate) const STAGE_PREFIX: &str = ".update-receipt-stage-";
/// Current receipt schema; bumped only for a change older readers cannot ignore.
const SCHEMA: u32 = 1;
/// Encoded payload bound before the installation/repository envelope.
pub(super) const MAX_BYTES: usize = 8192;

/// Exact identity of one prepared update under the authority that admitted it.
#[derive(Clone, Deserialize, Serialize)]
struct Record {
    schema_version: u32,
    installation: sugarsubstitute_contracts::installation::InstallationId,
    installation_directory: (u32, u32, u32),
    generation: Generation,
    generation_directory: (u32, u32, u32),
    release: PreparedRelease,
}

/// A protected receipt read for the currently prepared candidate, with its file kept open.
/// It proves what was prepared, not that it may start: call [`TrustStore::admit_receipt`].
pub struct UpdateReceipt {
    record: Record,
    _file: File,
}
impl UpdateReceipt {
    /// Generation the receipt was written for.
    pub fn generation(&self) -> &Generation {
        &self.record.generation
    }

    /// Release facts recorded under the trust writer when preparation completed.
    pub fn release(&self) -> &PreparedRelease {
        &self.record.release
    }

    /// The receipt must name exactly these native directories and the current prepared choice.
    pub(in crate::setup_storage) fn require(
        &self,
        installation: &Directory,
        directory: &Directory,
        snapshot: &InstallationRecord,
    ) -> io::Result<()> {
        let record = &self.record;
        if &record.installation != snapshot.installation()
            || record.installation_directory != installation.identity()?
            || record.generation_directory != directory.identity()?
            || !matches!(snapshot.candidate(), Some(Candidate::Prepared { generation }) if generation == &record.generation)
        {
            return Err(invalid("Update receipt does not match this prepared application"));
        }
        Ok(())
    }
}

impl UpdateAuthorization<'_> {
    /// Record `release` for `generation` inside the commit hold that verified it (D21 step 3).
    /// Called before the prepared selection record is published; a retry after a later failure
    /// replaces the receipt atomically, so the newest admitted facts always win.
    pub(in crate::setup_storage) fn record_receipt(
        &self,
        release: &PreparedRelease,
        generation: &Generation,
        directory: &Directory,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        let installation = &self.trust._installation_root;
        let expected = installation.path().join("releases").join(generation.id.as_str());
        if directory.identity()? != Directory::open(&expected, false)?.identity()? {
            return Err(invalid("Update receipt generation directory differs"));
        }
        let record = Record {
            schema_version: SCHEMA,
            installation: self.trust.installation.clone(),
            installation_directory: installation.identity()?,
            generation: generation.clone(),
            generation_directory: directory.identity()?,
            release: release.clone(),
        };
        let payload = serde_json::to_vec(&record).map_err(io::Error::other)?;
        if payload.len() > MAX_BYTES {
            return Err(invalid("Update receipt exceeds its bound"));
        }
        let bytes = envelope::encode(
            envelope::Kind::UpdateReceipt,
            &self.trust.installation,
            &self.trust.repository,
            &payload,
        )?;
        protection::with(context, |protection| {
            // Receipts are written only under the trust writer this hold retains.
            staging::reclaim(
                directory,
                STAGE_PREFIX.trim_end_matches('-'),
                &[],
                protection,
                context,
            )?;
            let destination = directory.path().join(FILE);
            let mut write = write_port::begin(UpdateFile::Receipt, &destination);
            let staged = staging::unique_path(directory, STAGE_PREFIX.trim_end_matches('-'))?;
            let mut file = protection.file(&staged, context)?;
            check(context)?;
            write.bytes(&mut file, &bytes)?;
            files::sync(&file, context)?;
            drop(file);
            write.publish(|| staging::replace_record(&staged, &destination))?;
            protection.verify_file(&directory.read(FILE)?)
        })
    }
}

impl TrustStore {
    /// Read the receipt of the currently prepared candidate, for launch without fresh metadata.
    /// Returns `None` when no candidate is prepared or it has no receipt; both mean the accepted
    /// version starts. Reserve ordinary trust-operation capacity.
    /// # Errors
    /// Rejects a damaged, foreign, unprotected or mismatched receipt, a changed installation,
    /// cancellation and storage errors. A present but invalid receipt is never ignored.
    pub fn update_receipt(
        &self,
        context: &JobContext,
    ) -> Result<Option<UpdateReceipt>, TrustFailure> {
        let selection = Self::selection(self._installation_root.path(), context)?;
        if selection.installation() != &self.installation {
            return Err(invalid("Update receipt installation identity changed").into());
        }
        let Some(Candidate::Prepared { generation }) = selection.candidate() else {
            return Ok(None);
        };
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            check(context)?;
            let installation = Directory::open(self._installation_root.path(), false)?;
            if installation.identity()? != self._installation_root.identity()? {
                return Err(invalid("Update receipt installation changed").into());
            }
            let path = installation.path().join("releases").join(generation.id.as_str());
            protection.directory(&path, Mode::Existing, context)?;
            let directory = Directory::open(&path, false)?;
            let mut file = match directory.read(FILE) {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(cause) => return Err(cause.into()),
            };
            protection.verify_file(&file)?;
            let bytes = files::read_bounded(&mut file, envelope::Kind::UpdateReceipt.limit())?;
            let payload = envelope::decode(
                envelope::Kind::UpdateReceipt,
                &self.installation,
                &self.repository,
                &bytes,
            )?;
            let record: Record = serde_json::from_slice(payload).map_err(io::Error::other)?;
            let receipt = UpdateReceipt { record, _file: file };
            if receipt.record.schema_version != SCHEMA
                || receipt.record.release.component
                    != sugarsubstitute_contracts::update::Component::Application
            {
                return Err(invalid("Update receipt has an unsupported format").into());
            }
            receipt.require(&installation, &directory, &selection)?;
            check(context)?;
            Ok(Some(receipt))
        })
        .map_err(TrustFailure::from)
    }

    /// Bind `receipt` to `request` and retain the signed targets cached for that request, which
    /// judge any metadata newer than the receipt, for the reopening and the trial admission of
    /// one received candidate. Reserve `OFFER_LOOKUP_RESERVATION_BYTES` for this call; the result
    /// keeps `OFFER_RESERVATION_BYTES` claimed until it is dropped. Missing cached metadata is
    /// retained as absent, which refuses only when newer metadata was authenticated.
    /// # Errors
    /// Preserves corrupt or foreign cached bytes, cancellation and storage failures.
    pub fn receipt_authority(
        &self,
        receipt: UpdateReceipt,
        request: Request,
        context: &JobContext,
    ) -> Result<ReceiptAuthority, TrustFailure> {
        check(context)?;
        let memory = context.memory().claim(OFFER_RESERVATION_BYTES).map_err(io::Error::other)?;
        let targets = self.cached_inputs(request, context)?.map(|inputs| inputs.targets);
        Ok(ReceiptAuthority { receipt, request, targets, _memory: memory })
    }

    /// Decide under the trust reader whether `authority`'s receipt may start its update now
    /// (D22). Its request carries this engine's local facts for the user's current subscription;
    /// `now` is Unix UTC seconds from the caller's injected clock. Its retained targets are
    /// consulted only to judge metadata newer than the receipt, and only while they are the
    /// newest this installation authenticated. Reserve ordinary trust-operation capacity.
    /// # Errors
    /// Returns `ReceiptRefused` with the failing condition, or storage, lock and cancellation
    /// failures. Nothing is written; on any error the accepted version starts.
    pub fn admit_receipt(
        &self,
        authority: &ReceiptAuthority,
        now: i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        check(context)?;
        let ReceiptAuthority { receipt, request, targets, .. } = authority;
        self.protected(context, |protection| {
            let _reader = self.reader(protection, context)?;
            self.require_subscription(request, protection, context)?;
            let trust = self.read(protection)?;
            let history = self.read_releases(protection)?;
            trust
                .admit_prepared(receipt.release(), request, &history, targets.as_deref(), now)
                .map_err(TrustFailure::ReceiptRefused)?;
            check(context)?;
            Ok(())
        })
    }
}

/// A receipt bound to the request it is judged for, with the cached signed targets retained
/// alongside, so trial admission judges it again without another cache read. Targets that are
/// no longer the newest authenticated ones refuse rather than stand in for newer metadata.
pub struct ReceiptAuthority {
    receipt: UpdateReceipt,
    request: Request,
    targets: Option<Vec<u8>>,
    _memory: MemoryClaim,
}
impl ReceiptAuthority {
    /// The protected receipt this authority judges.
    pub fn receipt(&self) -> &UpdateReceipt {
        &self.receipt
    }
}
