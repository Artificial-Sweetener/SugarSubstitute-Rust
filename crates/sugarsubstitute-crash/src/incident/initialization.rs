// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish the first strict ledger using the actual fresh-directory observation.
use super::{IncidentRepository, IncidentStoreError, IncidentStoreState, codec_ledger::digest};
use sugarsubstitute_contracts::{execution::JobContext, incident::SummaryEvictions};
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Keep an uncertain first activation and its original flushed inode with this repository.
pub(super) struct Initialization {
    stage: Option<usize>,
    flushed: Option<DiagnosticReadFacts>,
    actual: Option<DiagnosticActivation>,
}
impl IncidentRepository {
    /// Resolve this operation through the same live authenticated entry namespace.
    /// # Errors
    /// Foreign, closed or changed namespace input rejects. Actual partial native
    /// outputs remain retained in this repository for checked retirement.
    pub fn resolve_namespace(
        &mut self,
        request: &ProtectedDiagnosticWriteRequest,
        context: &JobContext,
    ) -> Result<DiagnosticStoreOpening, IncidentStoreError> {
        Ok(self.storage.resolve_namespace(request, &self.operation, context)?)
    }
    /// Acquire actual writer exclusion and complete bounded inventory before publication.
    /// # Errors
    /// Failed exclusion, incomplete inventory or original control failure retains
    /// the opening and partial owners; it grants no publication or readback proof.
    pub fn acquire_writer(&mut self, context: &JobContext) -> Result<(), IncidentStoreError> {
        self.storage.acquire_writer(&self.operation, context)?;
        self.inventory(context)?;
        Ok(())
    }
    /// Validate the mandatory current ledger before using an existing protected store.
    /// # Errors
    /// Missing, unknown, foreign or corrupt ledger bytes fail closed. A retained first
    /// activation must be reconciled instead of treating its partial state as an opening.
    pub fn validate_store(
        &mut self,
        context: &JobContext,
    ) -> Result<SummaryEvictions, IncidentStoreError> {
        if self.initialization.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.ledger(context)
    }
    /// Publish an empty canonical ledger in an actually newly created protected store.
    /// The genesis observation and this repository must belong to the same original
    /// operation. Ordinary absence cannot authorize this publication.
    /// # Errors
    /// Rejects another transaction, nonempty inventory, changed creation observation,
    /// original control failure or uncertain write/rename/close. Retained stages and
    /// read pins remain with this repository. Creation is attempted once; an uncertain
    /// activation can only be checked through `reconcile_initialization`.
    pub fn initialize(
        &mut self,
        genesis: &mut DiagnosticStoreGenesis,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, IncidentStoreError> {
        if self.initialization.is_some()
            || self.pending.is_some()
            || self.preflight.is_some()
            || !self.reads.is_empty()
            || !self.stages.is_empty()
            || self.storage.snapshot().stages() != 0
            || self.storage.snapshot().reads() != 0
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let state = IncidentStoreState::Ledger(self.empty_ledger()?);
        let mut bytes = self.storage.record_buffer(&self.operation, context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        super::allocation::decode(&mut decode, || {
            super::codec_ledger::write_store_state(&state, &mut bytes)
        })?;
        drop(decode);
        // The one-use intent remains latched even if native creation returns a partial output.
        self.initialization = Some(Initialization { stage: None, flushed: None, actual: None });
        let stage = self.storage.stage_genesis(
            genesis,
            digest(bytes.as_bytes()),
            bytes.as_bytes().len() as u64,
            &self.operation,
            context,
        )?;
        self.stages.push(stage)?;
        self.initialization_mut()?.stage = Some(0);
        self.stages.get_mut(0)?.write_chunk(bytes.as_bytes(), context)?;
        let flushed = self.stages.get_mut(0)?.flush(context)?.read_facts();
        if flushed.sha256() != digest(bytes.as_bytes())
            || flushed.bytes() != bytes.as_bytes().len() as u64
        {
            return Err(IncidentStoreError::WrongDigest);
        }
        self.initialization_mut()?.flushed = Some(flushed);
        drop(bytes);
        let actual = self.storage.activate(
            self.stages.get_mut(0)?,
            DiagnosticPredecessor::Absent,
            &self.operation,
            context,
        )?;
        self.adopt_initialization(actual)?;
        self.finish_initialization(context)
    }
    /// Check the same uncertain first activation using only its retained actual inode.
    /// This creates no stage, replaces no predecessor and renews no deadline.
    /// # Errors
    /// Rejects incomplete writes, absent/changed active ledger, failed checked close or
    /// original control failure. All partial owners remain retained for actual cleanup.
    pub fn reconcile_initialization(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, IncidentStoreError> {
        let pending = self.initialization_mut()?;
        if pending.actual.is_none() {
            let facts = pending.flushed.ok_or(IncidentStoreError::RecoveryConflict)?;
            let actual = self.storage.reconcile_activation(
                DiagnosticObject::StoreState,
                super::repository::predecessor(facts),
                &self.operation,
                context,
            )?;
            self.adopt_initialization(actual)?;
        }
        self.finish_initialization(context)
    }
    fn initialization_mut(&mut self) -> Result<&mut Initialization, IncidentStoreError> {
        self.initialization.as_mut().ok_or(IncidentStoreError::RecoveryConflict)
    }
    fn adopt_initialization(
        &mut self,
        actual: DiagnosticActivation,
    ) -> Result<(), IncidentStoreError> {
        let pending = self.initialization_mut()?;
        if actual.object() != DiagnosticObject::StoreState
            || Some(actual.facts()) != pending.flushed
        {
            return Err(IncidentStoreError::WrongDigest);
        }
        pending.actual = Some(actual);
        if let Some(index) = pending.stage.take() {
            self.stages.remove(index)?;
        }
        Ok(())
    }
    fn finish_initialization(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, IncidentStoreError> {
        // Retry a failed actual read close before acquiring another read slot.
        while !self.reads.is_empty() {
            self.close_read(0, context)?;
        }
        if self.ledger(context)? != self.empty_ledger()? {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let actual =
            self.initialization_mut()?.actual.take().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.initialization = None;
        Ok(actual)
    }
    fn empty_ledger(&self) -> Result<SummaryEvictions, IncidentStoreError> {
        Ok(SummaryEvictions::new(self.operation.key().owner().namespace, 0, 0, 0, None)?)
    }
}
