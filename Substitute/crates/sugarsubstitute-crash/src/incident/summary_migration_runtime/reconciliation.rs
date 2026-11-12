// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconcile only exact actual migration archives/intents before a fresh schema step or publication.
use super::*;
impl IncidentRepository {
    /// Validate every archive against its preserved original; unresolved intents grant no publication.
    pub(in crate::incident) fn recognize_summary_archives(
        &mut self,
        inventory: &DiagnosticInventory,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Journal)
        {
            let (bytes, _, index) = self.read_candidate(candidate, context)?;
            let (operation, origin) = {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                super::super::allocation::decode(&mut decode, || {
                    match super::super::decode_store_state(bytes.as_bytes())? {
                        IncidentStoreState::SummaryMigrationArchive(archive) => {
                            Ok((archive.migration().operation(), false))
                        }
                        IncidentStoreState::RecoveryCaptureOrigin(record) => {
                            Ok((record.operation(), true))
                        }
                        _ => Err(IncidentStoreError::RecoveryConflict),
                    }
                })?
            };
            self.reads
                .get_mut(index)?
                .bind_object(DiagnosticObject::Journal(operation), context)?;
            drop(bytes);
            if origin {
                self.storage.recognize_recovery_capture_origin(
                    self.reads.get_mut(index)?,
                    &super::super::IncidentRecoveryCaptureOriginValidator,
                    &self.operation,
                    context,
                )?;
            } else {
                self.storage.recognize_summary_migration_archive(
                    self.reads.get_mut(index)?,
                    &IncidentSummaryMigrationValidator,
                    &self.operation,
                    context,
                )?;
            }
            self.close_read(index, context)?;
        }
        self.storage.validate_preserved_migration_inventory(&self.operation, context)?;
        Ok(())
    }
    /// Finish at most one exact persisted intent before any new migration under this operation.
    pub(super) fn reconcile_summary_migrations(
        &mut self,
        inventory: &DiagnosticInventory,
        work: &mut Option<SummaryMigrationWork>,
        context: &JobContext,
    ) -> Result<bool, IncidentStoreError> {
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Journal)
        {
            let (bytes, _, index) = self.read_candidate(candidate, context)?;
            let journal = {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                super::super::allocation::decode(&mut decode, || {
                    match super::super::decode_store_state(bytes.as_bytes())? {
                        IncidentStoreState::SummaryMigration(journal) => Ok(Ok(*journal)),
                        IncidentStoreState::SummaryMigrationArchive(archive) => {
                            Ok(Err((archive.migration().operation(), false)))
                        }
                        IncidentStoreState::RecoveryCaptureOrigin(record) => {
                            Ok(Err((record.operation(), true)))
                        }
                        _ => Err(IncidentStoreError::RecoveryConflict),
                    }
                })?
            };
            drop(bytes);
            let journal = match journal {
                Err((operation, origin)) => {
                    self.reads
                        .get_mut(index)?
                        .bind_object(DiagnosticObject::Journal(operation), context)?;
                    if origin {
                        self.storage.recognize_recovery_capture_origin(
                            self.reads.get_mut(index)?,
                            &super::super::IncidentRecoveryCaptureOriginValidator,
                            &self.operation,
                            context,
                        )?;
                    } else {
                        self.storage.recognize_summary_migration_archive(
                            self.reads.get_mut(index)?,
                            &IncidentSummaryMigrationValidator,
                            &self.operation,
                            context,
                        )?;
                    }
                    self.close_read(index, context)?;
                    continue;
                }
                Ok(journal) => journal,
            };
            self.reads
                .get_mut(index)?
                .bind_object(DiagnosticObject::Journal(journal.operation()), context)?;
            let read = self.reads.remove(index)?;
            *work = Some(SummaryMigrationWork {
                operation: self.operation(),
                original_read: None,
                recovery_journal: Some(read),
                recovery_summary: None,
                preparation: None,
                journal: Some(journal),
                journal_stage: None,
                permit: None,
                activation: None,
                archive: None,
                completed: false,
            });
            let work = work.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            let journal = work.journal.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
            work.recovery_summary = Some(self.storage.read(
                DiagnosticObject::Summary(journal.incident()),
                65536,
                &self.operation,
                context,
            )?);
            let summary =
                work.recovery_summary.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            let original = summary.facts().file_identity() == journal.original_file().identity()
                && summary.facts().sha256() == journal.original_file().sha256()
                && summary.facts().bytes() == journal.original_file().bytes();
            work.permit = Some(self.storage.recover_summary_migration(
                work.recovery_journal.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                summary,
                &IncidentSummaryMigrationValidator,
                &self.operation,
                context,
            )?);
            if original {
                self.storage.recover_summary_migration_candidate(
                    work.permit.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                    &IncidentSummaryMigrationValidator,
                    &self.operation,
                    context,
                )?;
            }
            self.finish_summary_migration_work(work, context)?;
            work.recovery_journal
                .as_mut()
                .ok_or(IncidentStoreError::RecoveryConflict)?
                .retire(context)?;
            work.recovery_summary
                .as_mut()
                .ok_or(IncidentStoreError::RecoveryConflict)?
                .retire(context)?;
            work.completed = true;
            return Ok(true);
        }
        Ok(false)
    }
}
