// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep a damaged or missing selection record, or a missing accepted release, from blocking a
//! launch.
//!
//! Every launch engine reconciles the record before routing, under the selection writer. A
//! readable record refreshes a last known good copy beside it. A record that is missing or no
//! longer decodes is moved aside with its exact bytes and rebuilt from that copy: acceptance,
//! the retained release, strikes, rollback and repair are kept and any candidate is dropped, so
//! only a release this installation accepted can start, and the next update check prepares the
//! candidate again. Without a readable copy the record is left as it is and its failure stands.
//! A record written in another schema is never treated as damage.
//!
//! An accepted release whose files are gone is replaced by the retained release, as when its
//! engine cannot start, and recorded for repair. Every start still verifies the chosen release's
//! payload in full; these checks only decide which accepted release is worth trying.
use super::*;
use crate::setup_storage::application::VerifiedApplication;
use std::sync::Arc;
use sugarsubstitute_contracts::installation::{Generation, Rejection};

/// Last known good copy of the record, written only from a record that decoded.
const LAST_GOOD: &str = ".substitute-selection.last-good.json";
/// The most recent damaged record, moved aside with its exact bytes for diagnosis.
const DAMAGED: &str = ".substitute-selection.damaged.json";
/// Stage prefix for the copy's atomic replacement.
const COPY_STAGE: &str = ".selection-copy-stage";

/// What the reconciliation did with the record.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::setup_storage) enum Reconciled {
    /// The record was readable; its copy is current.
    Readable,
    /// The record was damaged or missing and was rebuilt from its copy.
    Restored,
    /// The record cannot be rebuilt here: no copy exists, the copy is unreadable, or the record
    /// belongs to another schema. It is left untouched.
    Untouched,
}

/// Make the installation's record readable again from its copy when it is damaged or missing,
/// and refresh the copy when it is readable. An installation without established selection is
/// left alone. Run on an admitted storage worker before the record is opened for routing.
/// # Errors
/// Lock, cancellation, protection and storage failures; the record is unchanged after an error
/// unless the damaged record was already moved aside, which the next reconciliation completes.
pub(in crate::setup_storage) fn reconcile(
    installation: &Path,
    context: &JobContext,
) -> Result<Reconciled, SelectionFailure> {
    let Some(store) = SelectionStore::open_for_restoration(installation, context)? else {
        return Ok(Reconciled::Untouched);
    };
    store.protected(context, |protection| {
        let _writer = writer(&store.root, protection, context)?;
        let damaged = match store.read(protection) {
            Ok(Some(record)) => {
                let bytes = record.encode()?;
                if read_bounded(&store.root, LAST_GOOD, protection)?.as_deref()
                    != Some(bytes.as_slice())
                {
                    write_copy(&store.root, LAST_GOOD, &bytes, protection, context)?;
                }
                return Ok(Reconciled::Readable);
            }
            Ok(None) => false,
            Err(SelectionFailure::Record(Rejection::Schema)) => {
                return Ok(Reconciled::Untouched);
            }
            Err(SelectionFailure::Record(_)) => true,
            Err(cause) if invalid_data(&cause) || oversized(&store.root, context) => true,
            Err(cause) => return Err(cause),
        };
        let Some(copy) = read_copy(&store.root, protection) else {
            tracing::warn!(
                operation = "installation_selection_unrecoverable",
                damaged,
                "The installation record is unreadable and has no last known good copy"
            );
            return Ok(Reconciled::Untouched);
        };
        let restored = store.choose_present(copy.restored()?, context);
        if damaged {
            let aside = store.root.path().join(DAMAGED);
            write_port::begin(UpdateFile::Selection, &aside)
                .publish(|| staging::replace_record(&store.root.path().join(RECORD), &aside))?;
        }
        // Only this writer stages the record, so no earlier stage is still live.
        staging::reclaim(&store.root, RECORD_STAGE, &[], protection, context)?;
        store.publish(protection, &restored, context)?;
        write_copy(&store.root, LAST_GOOD, &restored.encode()?, protection, context)?;
        tracing::warn!(
            operation = "installation_selection_restored",
            installation = restored.installation().as_str(),
            revision = restored.revision(),
            damaged,
            accepted = restored.accepted().map(|generation| generation.id.as_str()),
            "Rebuilt the installation record from its last known good copy"
        );
        Ok(Reconciled::Restored)
    })
}

impl SelectionStore {
    /// Pin an established namespace without requiring its record to decode, for reconciliation.
    /// `None` before selection has been enabled for this installation.
    fn open_for_restoration(
        root: &Path,
        context: &JobContext,
    ) -> Result<Option<Self>, SelectionFailure> {
        let (installation, ownership, memory) = Self::installation(root, context)?;
        let Some(root) = namespace::open_established(&installation, context)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            root,
            _installation: installation,
            _ownership: ownership,
            _namespace_marker: None,
            _memory: memory,
            clock: StrikeClock::System,
        }))
    }

    /// Run the retained release instead of an accepted release whose files are gone.
    fn choose_present(
        &self,
        record: InstallationRecord,
        context: &JobContext,
    ) -> InstallationRecord {
        let Some(accepted) = record.accepted().cloned() else { return record };
        if self.present(&accepted, context) {
            return record;
        }
        match record.retained() {
            Some(retained) if self.present(retained, context) => {
                record.fall_back_to_retained(&accepted).unwrap_or(record)
            }
            _ => record,
        }
    }

    /// Whether `generation`'s inventory is still on disk and matches the record. Only absence or
    /// a different inventory counts as gone; a held or unreadable file keeps the release.
    fn present(&self, generation: &Generation, context: &JobContext) -> bool {
        match VerifiedApplication::inventory_matches(&self._installation, generation, context) {
            Ok(matches) => matches,
            Err(cause) => {
                tracing::warn!(
                    operation = "installation_release_presence_unknown",
                    generation = generation.id.as_str(),
                    cause = %cause,
                    "Could not inspect a release's inventory; keeping it selected"
                );
                true
            }
        }
    }

    /// Before routing, run the retained release when the accepted release's files are gone.
    /// A live trial, a busy run or an accepted release still present leaves `record` as it is.
    /// # Errors
    /// Lock, cancellation and storage failures while publishing the fallback.
    pub(in crate::setup_storage) fn fall_back_from_missing(
        self: &Arc<Self>,
        record: InstallationRecord,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        let Some(accepted) = record.accepted() else { return Ok(record) };
        if matches!(record.candidate(), Some(Candidate::Trial { .. }))
            || self.present(accepted, context)
            || !record.retained().is_some_and(|retained| self.present(retained, context))
        {
            return Ok(record);
        }
        let run = match InstallationRun::acquire(Arc::clone(self), context) {
            Ok(run) => run,
            Err(SelectionFailure::RunBusy(_)) => return Ok(record),
            Err(cause) => return Err(cause),
        };
        run.fall_back_to_retained(accepted, context)
    }
}

/// Whether a storage failure reports bytes that are not a valid record, such as a truncated file.
fn invalid_data(cause: &SelectionFailure) -> bool {
    matches!(cause, SelectionFailure::Storage(cause)
        if matches!(cause.kind(), io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof))
}

/// Whether the record file is larger than any record can be, which no writer produces.
fn oversized(root: &Directory, context: &JobContext) -> bool {
    root.read(RECORD)
        .and_then(|file| files::length(&file, context))
        .is_ok_and(|length| length > MAX_RECORD_BYTES as u64)
}

/// The copy, when it is protected and decodes; anything else is no copy.
fn read_copy(root: &Directory, protection: &Protection) -> Option<InstallationRecord> {
    let bytes = match read_bounded(root, LAST_GOOD, protection) {
        Ok(bytes) => bytes?,
        Err(cause) => {
            tracing::warn!(
                operation = "installation_selection_copy_unreadable",
                cause = %cause,
                "The installation record's last known good copy cannot be read"
            );
            return None;
        }
    };
    InstallationRecord::decode(&bytes).ok()
}

/// Read a protected sibling of the record, or `None` when it does not exist.
fn read_bounded(
    root: &Directory,
    leaf: &str,
    protection: &Protection,
) -> io::Result<Option<Vec<u8>>> {
    let mut file = match root.read(leaf) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    protection.verify_file(&file)?;
    files::read_bounded(&mut file, MAX_RECORD_BYTES).map(Some)
}

/// Replace a protected sibling of the record atomically with `bytes`. The caller holds the
/// selection writer, so a stage an interrupted earlier copy left behind is dead and reclaimed.
fn write_copy(
    root: &Directory,
    leaf: &str,
    bytes: &[u8],
    protection: &mut Protection,
    context: &JobContext,
) -> Result<(), SelectionFailure> {
    staging::reclaim(root, COPY_STAGE, &[], protection, context)?;
    let destination = root.path().join(leaf);
    let mut write = write_port::begin(UpdateFile::Selection, &destination);
    let staged = staging::unique_path(root, COPY_STAGE)?;
    let mut file = protection.file(&staged, context)?;
    write.bytes(&mut file, bytes)?;
    files::sync(&file, context)?;
    drop(file);
    check(context)?;
    write.publish(|| staging::replace_record(&staged, &destination))?;
    Ok(())
}

#[cfg(test)]
mod tests;
