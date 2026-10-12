// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain streaming publication stages before native creation and after uncertainty.
use super::*;

pub(super) struct RetainedStage {
    pub(super) id: u64,
    pub(super) native: Option<NativeId>,
    pub(super) object: DiagnosticObject,
    pub(super) reserved: u64,
    pub(super) written: u64,
    pub(super) flushed: Option<DiagnosticReadFacts>,
    pub(super) recovered: Option<RecordedDiagnosticFileFacts>,
    pub(super) recovery_predecessor: Option<DiagnosticPredecessor>,
    pub(super) migration: Option<summary_migration::MigrationRole>,
    pub(super) activated: bool,
}

/// Pin one owner-retained stage; dropping the pin preserves its charge and custody.
pub struct DiagnosticStage {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Observe checked actual flushed stage bytes without granting activation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticStageFacts(pub(super) DiagnosticReadFacts);
impl DiagnosticStageFacts {
    /// Observe actual flushed identity, digest and length.
    pub const fn read_facts(self) -> DiagnosticReadFacts {
        self.0
    }
}
/// Retain a victim-bound journal stage distinct from ordinary publication.
pub struct DiagnosticDispositionStage {
    pub(super) stage: DiagnosticStage,
    pub(super) plan: DiagnosticDispositionPlan,
}
impl DiagnosticStorage {
    /// Reserve bounded custody before creating any publication inode.
    /// # Errors
    /// Rejects incomplete inventory, absent exclusion, foreign object, saturated
    /// stages or charged bytes, and expired context. Partial creation stays owned.
    pub fn stage(
        &mut self,
        object: DiagnosticObject,
        reserved_bytes: u64,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStage, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        mutation(&state, operation.key)?;
        let group = match object {
            DiagnosticObject::Payload(key, ..) => {
                Some(crate::protected_storage::record_file::diagnostic_subject_digest(key))
            }
            _ => None,
        };
        if let Some(group) = group
            && !state.payload_groups.contains(&group)
        {
            if state.payload_groups.len() >= sugarsubstitute_contracts::incident::MAX_FULL_INCIDENTS
            {
                return Err(error(operation.key, Code::Capacity));
            }
            state.payload_groups.try_reserve_exact(1).map_err(|cause| {
                native(
                    operation.key,
                    Code::Capacity,
                    ClaimError::new("diagnostic_group_reservation", cause),
                )
            })?;
        }
        if object.namespace().is_some_and(|value| Some(value) != state.namespace)
            || reserved_bytes == 0
            || reserved_bytes > object.bound()
            || state.stages.len() >= DIAGNOSTIC_STAGE_BOUND
        {
            return Err(error(operation.key, Code::Capacity));
        }
        let charge = state
            .stages
            .iter()
            .try_fold(state.charged_bytes, |sum, value| sum.checked_add(value.reserved))
            .and_then(|sum| sum.checked_add(reserved_bytes));
        if charge.is_none_or(|total| total > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES)
        {
            return Err(error(operation.key, Code::Capacity));
        }
        let (slots, bound) = match object {
            DiagnosticObject::Seed(_) => (&state.seeds, DIAGNOSTIC_SEED_BOUND),
            DiagnosticObject::Summary(_) => {
                (&state.summaries, sugarsubstitute_contracts::incident::MAX_SUMMARIES)
            }
            _ => (&state.seeds, usize::MAX),
        };
        if let DiagnosticObject::Seed(key) | DiagnosticObject::Summary(key) = object {
            let digest = crate::protected_storage::record_file::diagnostic_subject_digest(key);
            let pending = state
                .stages
                .iter()
                .filter(|stage| match (object, stage.object) {
                    (DiagnosticObject::Seed(_), DiagnosticObject::Seed(key))
                    | (DiagnosticObject::Summary(_), DiagnosticObject::Summary(key)) => !slots
                        .contains(
                            &crate::protected_storage::record_file::diagnostic_subject_digest(key),
                        ),
                    _ => false,
                })
                .count();
            if !slots.contains(&digest) && slots.len() + pending >= bound {
                return Err(error(operation.key, Code::Capacity));
            }
        }
        let id = next_lease(&mut state, operation.key)?;
        if let Some(group) = group
            && !state.payload_groups.contains(&group)
        {
            state.payload_groups.push(group);
        }
        state.stages.push(RetainedStage {
            id,
            native: None,
            object,
            reserved: reserved_bytes,
            written: 0,
            flushed: None,
            recovered: None,
            recovery_predecessor: None,
            migration: None,
            activated: false,
        });
        let native_id = state
            .port
            .create_stage(object, operation.key, reserved_bytes, context)
            .map_err(|cause| native(operation.key, Code::Write, cause))?;
        let index = state.stages.len() - 1;
        state.stages[index].native = Some(native_id);
        if native_id == 0 {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        Ok(DiagnosticStage { shared: self.shared.clone(), operation: operation.key, id })
    }
    /// Stage the exact canonical journal digest before obtaining deletion authority.
    /// # Errors
    /// Rejects a foreign original operation or ordinary stage admission failure.
    pub fn stage_disposition(
        &mut self,
        plan: DiagnosticDispositionPlan,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDispositionStage, DiagnosticStorageError> {
        if plan.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let stage =
            self.stage(DiagnosticObject::Journal(plan.operation), 65536, operation, context)?;
        Ok(DiagnosticDispositionStage { stage, plan })
    }
}
impl DiagnosticStage {
    /// Append at most64KiB within the original reservation; writing invalidates flush proof.
    /// # Errors
    /// Native partial writes preserve stage custody and prevent later activation until
    /// a complete actual length/hash flush matches the admitted stream.
    pub fn write_chunk(
        &mut self,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = lock(&self.shared);
        check(&mut state, self.operation, context, false)?;
        mutation(&state, self.operation)?;
        let index = state
            .stages
            .iter()
            .position(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let stage = &mut state.stages[index];
        if stage.recovered.is_some() || stage.migration.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        let end = stage
            .written
            .checked_add(input.len() as u64)
            .filter(|end| *end <= stage.reserved && input.len() <= DIAGNOSTIC_IO_BYTES)
            .ok_or_else(|| error(self.operation, Code::Capacity))?;
        let id = stage.native.ok_or_else(|| error(self.operation, Code::Closed))?;
        let offset = stage.written;
        stage.flushed = None;
        state
            .port
            .write(id, offset, input, context)
            .map_err(|cause| native(self.operation, Code::Write, cause))?;
        state.stages[index].written = end;
        check(&mut state, self.operation, context, false)
    }
    /// Flush and read back the actual entire stream before publishing exact facts.
    /// # Errors
    /// Short, changed, unflushed or oversized bytes retain the stage without proof.
    pub fn flush(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticStageFacts, DiagnosticStorageError> {
        let mut state = lock(&self.shared);
        check(&mut state, self.operation, context, false)?;
        if state.migration.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        let index = state
            .stages
            .iter()
            .position(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let id = state.stages[index].native.ok_or_else(|| error(self.operation, Code::Closed))?;
        let facts = state
            .port
            .flush(id, context)
            .map_err(|cause| native(self.operation, Code::Flush, cause))?;
        if facts.identity == [0; 24]
            || facts.sha256 == [0; 32]
            || facts.bytes != state.stages[index].written
            || state.stages[index].recovered.is_some_and(|expected| !expected.matches(facts))
        {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        check(&mut state, self.operation, context, false)?;
        state.stages[index].flushed = Some(facts);
        Ok(DiagnosticStageFacts(facts))
    }
    /// Checked-close custody while preserving unactivated persistent stage bytes.
    /// # Errors
    /// Failed close retains the slot and original reservation.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        let mut state = lock(&self.shared);
        check(&mut state, self.operation, context, false)?;
        let Some(index) = state.stages.iter().position(|value| value.id == self.id) else {
            return Ok(());
        };
        if state.stages[index].migration.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        if let Some(id) = state.stages[index].native {
            state
                .port
                .close(id, context)
                .map_err(|cause| native(self.operation, Code::Retirement, cause))?;
        }
        state.stages.remove(index);
        state.inventory_complete = false;
        Ok(())
    }
}
impl DiagnosticDispositionStage {
    /// Append bounded canonical journal bytes; the repository validates their schema.
    /// # Errors
    /// Preserves ordinary stage failures and their native causes.
    pub fn write_chunk(
        &mut self,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        self.stage.write_chunk(input, context)
    }
    /// Observe checked actual bytes before exact journal-digest activation.
    /// # Errors
    /// Rejects a complete actual digest differing from the victim-bound plan.
    pub fn flush(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticStageFacts, DiagnosticStorageError> {
        let facts = self.stage.flush(context)?;
        if facts.0.sha256 != self.plan.journal_sha256 {
            return Err(error(self.stage.operation, Code::ChangedIdentity));
        }
        Ok(facts)
    }
    /// Checked-close this journal stage while preserving its bytes.
    /// # Errors
    /// Retains the stage and charge after closure failure.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        self.stage.retire(context)
    }
}
pub(super) fn mutation(
    state: &State,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if !state.writer {
        return Err(error(key, Code::Busy));
    }
    if state.migration.is_some() {
        return Err(error(key, Code::Busy));
    }
    if !state.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    if state.port.live_handles() >= DIAGNOSTIC_HANDLE_BOUND {
        return Err(error(key, Code::Capacity));
    }
    Ok(())
}
