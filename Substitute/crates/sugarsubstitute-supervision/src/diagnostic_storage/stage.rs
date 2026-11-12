// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain streaming publication stages before native creation and after uncertainty.
use super::*;
pub(super) mod slots;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StageRole {
    Migration(summary_migration::MigrationRole),
    PayloadCandidate,
    PayloadJournal,
}

pub(super) struct RetainedStage {
    pub(super) id: u64,
    pub(super) native: Option<NativeId>,
    pub(super) object: DiagnosticObject,
    pub(super) reserved: u64,
    pub(super) written: u64,
    pub(super) flushed: Option<DiagnosticReadFacts>,
    pub(super) recovered: Option<RecordedDiagnosticFileFacts>,
    pub(super) recovery_predecessor: Option<DiagnosticPredecessor>,
    pub(super) role: Option<StageRole>,
    pub(super) activated: bool,
    pub(super) genesis: Option<genesis::GenesisContent>,
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
        self.stage_with_purpose(object, reserved_bytes, None, operation, context)
    }
    pub(super) fn stage_with_purpose(
        &mut self,
        object: DiagnosticObject,
        reserved_bytes: u64,
        genesis: Option<genesis::GenesisContent>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStage, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        mutation(&mut state, operation.key)?;
        if let Some(content) = genesis {
            genesis::validate_issue(&mut state, operation.key, content)?;
        }
        let group =
            crate::durable_storage::protected::record_file::diagnostic_payload_group_digest(object);
        if let Some(group) = group
            && !state.data_mut()?.payload_groups.contains(&group)
        {
            if state.data_mut()?.payload_groups.len()
                >= sugarsubstitute_contracts::incident::MAX_FULL_INCIDENTS
            {
                return Err(error(operation.key, Code::Capacity));
            }
            let reservation =
                allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
            state
                .data_mut()?
                .payload_groups
                .try_reserve_exact(1)
                .map_err(|cause| reservation.failed(cause))?;
        }
        let namespace = state.data_mut()?.namespace;
        if object.namespace().is_some_and(|value| Some(value) != namespace)
            || reserved_bytes == 0
            || reserved_bytes > object.bound()
            || state.data_mut()?.stage_slots() >= DIAGNOSTIC_STAGE_BOUND
        {
            return Err(error(operation.key, Code::Capacity));
        }
        {
            let data = state.data_mut()?;
            let charge = data
                .stages
                .iter()
                .try_fold(data.charged_bytes, |sum, value| sum.checked_add(value.reserved))
                .and_then(|sum| sum.checked_add(reserved_bytes));
            if charge
                .is_none_or(|total| total > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES)
            {
                return Err(error(operation.key, Code::Capacity));
            }
            let (slots, bound) = match object {
                DiagnosticObject::Seed(_) => (&data.seeds, DIAGNOSTIC_SEED_BOUND),
                DiagnosticObject::Summary(_) => {
                    (&data.summaries, sugarsubstitute_contracts::incident::MAX_SUMMARIES)
                }
                _ => (&data.seeds, usize::MAX),
            };
            if let DiagnosticObject::Seed(key) | DiagnosticObject::Summary(key) = object {
                let digest =
                    crate::durable_storage::protected::record_file::diagnostic_subject_digest(key);
                let pending = data
                    .stages
                    .iter()
                    .filter(|stage| match (object, stage.object) {
                        (DiagnosticObject::Seed(_), DiagnosticObject::Seed(key))
                        | (DiagnosticObject::Summary(_), DiagnosticObject::Summary(key)) => !slots
                            .contains(
                                &crate::durable_storage::protected::record_file::diagnostic_subject_digest(
                                    key,
                                ),
                            ),
                        _ => false,
                    })
                    .count();
                if !slots.contains(&digest) && slots.len() + pending >= bound {
                    return Err(error(operation.key, Code::Capacity));
                }
            }
        }
        let id = next_lease(&mut state, operation.key)?;
        if let Some(content) = genesis {
            let retained = state
                .data_mut()?
                .genesis
                .as_mut()
                .ok_or_else(|| error(operation.key, Code::InvalidIdentity))?;
            retained.stage = Some(id);
            retained.content = Some(content);
        }
        if let Some(group) = group
            && !state.data_mut()?.payload_groups.contains(&group)
        {
            state.data_mut()?.payload_groups.push(group);
        }
        state.data_mut()?.stages.push(RetainedStage {
            id,
            native: None,
            object,
            reserved: reserved_bytes,
            written: 0,
            flushed: None,
            recovered: None,
            recovery_predecessor: None,
            role: None,
            activated: false,
            genesis,
        });
        let native_id = state
            .call_native(|port| port.create_stage(object, operation.key, reserved_bytes, context))?
            .map_err(|cause| native(operation.key, Code::Write, cause))?;
        let index = state.data_mut()?.stages.len() - 1;
        state.data_mut()?.stages[index].native = Some(native_id);
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
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        mutation(&mut state, self.operation)?;
        let index = state
            .data_mut()?
            .stages
            .iter()
            .position(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let stage = &mut state.data_mut()?.stages[index];
        if stage.recovered.is_some() || stage.role.is_some() {
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
            .call_native(|port| port.write(id, offset, input, context))?
            .map_err(|cause| native(self.operation, Code::Write, cause))?;
        state.data_mut()?.stages[index].written = end;
        check(&mut state, self.operation, context, false)
    }
    /// Flush and read back the actual entire stream before publishing exact facts.
    /// # Errors
    /// Short, changed, unflushed or oversized bytes retain the stage without proof.
    pub fn flush(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticStageFacts, DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        if state.data_mut()?.migration.is_some() || state.data_mut()?.payload.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        let index = state
            .data_mut()?
            .stages
            .iter()
            .position(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let id = state.data_mut()?.stages[index]
            .native
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let facts = state
            .call_native(|port| port.flush(id, context))?
            .map_err(|cause| native(self.operation, Code::Flush, cause))?;
        if facts.identity == [0; 24]
            || facts.sha256 == [0; 32]
            || facts.bytes != state.data_mut()?.stages[index].written
            || state.data_mut()?.stages[index]
                .recovered
                .is_some_and(|expected| !expected.matches(facts))
        {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        check(&mut state, self.operation, context, false)?;
        state.data_mut()?.stages[index].flushed = Some(facts);
        Ok(DiagnosticStageFacts(facts))
    }
    /// Checked-close custody while preserving unactivated persistent stage bytes.
    /// # Errors
    /// Failed close retains the slot and original reservation.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let Some(index) = state.data_mut()?.stages.iter().position(|value| value.id == self.id)
        else {
            return Ok(());
        };
        if state.data_mut()?.stages[index].role.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        if let Some(id) = state.data_mut()?.stages[index].native {
            state
                .call_native(|port| port.close(id, context))?
                .map_err(|cause| native(self.operation, Code::Retirement, cause))?;
        }
        state.data_mut()?.stages.remove(index);
        state.data_mut()?.inventory_complete = false;
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
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if !state.data_mut()?.writer {
        return Err(error(key, Code::Busy));
    }
    if state.data_mut()?.migration.is_some()
        || state.data_mut()?.payload.is_some()
        || state.data_mut()?.capture.is_some()
    {
        return Err(error(key, Code::Busy));
    }
    if !state.data_mut()?.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    if state.data_mut()?.live_handles() >= DIAGNOSTIC_HANDLE_BOUND {
        return Err(error(key, Code::Capacity));
    }
    Ok(())
}
