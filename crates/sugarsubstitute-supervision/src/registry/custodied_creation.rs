// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original managed creation alive through actual prepared-Job and root receipts.
use super::*;
use crate::{
    claims::managed_custody::ManagedCustodySource,
    native_process::{FamilyStorage, custody_transfer::CustodyDestination},
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyCommand as Command, BackendCustodyReceipt as Receipt},
    execution::{MemoryClaim, Moment},
};
use sugarsubstitute_execution::backend_custody::process::{
    CustodiedProcessDriver, CustodiedProcessMemory, CustodiedProcessProgress, CustodiedProcessWake,
};

/// Complete captured input data remains covered before its original transient claim.
struct Input {
    value: WindowsLaunchInput,
    _memory: MemoryClaim,
}
enum Phase {
    Prepared,
    WaitingPrepared,
    WaitingRoot,
    Started,
}
pub(super) struct Driver {
    shared: SharedOwner,
    input: Option<Input>,
    family: Option<FamilyStorage>,
    destination: Option<CustodyDestination>,
    queued: Option<Command>,
    nonce: Option<u64>,
    facts: Option<CreationFacts>,
    memory: Option<CustodiedProcessMemory>,
    phase: Phase,
    client: ExecutionClient,
    ticket: LaunchTicket,
    source: ManagedCustodySource,
}
impl Driver {
    /// Claim real input capacities before constructing the actual retained driver.
    pub(super) fn construct(
        shared: SharedOwner,
        mut input: WindowsLaunchInput,
        context: &JobContext,
        memory: &CustodiedProcessMemory,
        client: ExecutionClient,
        ticket: LaunchTicket,
    ) -> Result<Box<dyn CustodiedProcessDriver>, NativeObservationError> {
        let source = input
            .managed_custody
            .take()
            .ok_or_else(|| native(RegistryError::invalid("custodied_source")))?;
        if ticket.binding != FamilyBinding::Managed(source.resource())
            || !source.parent().matches_execution(&client)
        {
            return Err(native(RegistryError::invalid("custodied_source_association")));
        }
        let transient =
            input.required_reservation_bytes() - crate::registry_input::FAMILY_STORAGE_BYTES;
        let claim = context
            .memory()
            .claim(transient)
            .map_err(|cause| native(RegistryError::new("custodied_input_memory", cause)))?;
        let family = Family::custodied(memory, size_of::<Self>()).map_err(native)?;
        let destination = CustodyDestination::new(source.parent());
        Ok(Box::new(Self {
            shared,
            input: Some(Input { value: input, _memory: claim }),
            family: Some(family),
            destination: Some(destination),
            queued: None,
            nonce: None,
            facts: None,
            memory: Some(memory.clone()),
            phase: Phase::Prepared,
            client,
            ticket,
            source,
        }))
    }
    /// Observe current original controls before each fresh native or protocol effect.
    fn check(&self) -> Result<(), RegistryError> {
        if self.shared.admission.load(Ordering::Acquire) == CLOSED {
            return Err(RegistryError::invalid("custodied_closed"));
        }
        self.memory
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("custodied_memory"))?
            .check_effect()
            .map_err(|cause| RegistryError::new("custodied_original_control", cause))?;
        self.source
            .parent()
            .check()
            .map_err(|cause| RegistryError::new("custodied_parent_control", cause))?;
        Ok(())
    }
    /// Issue a retained original command only when that controller transaction is available.
    fn send(&mut self, context: &JobContext) -> Result<(), RegistryError> {
        let Some(command) = self.queued else {
            return Ok(());
        };
        self.check()?;
        let until = context.deadline().ok_or_else(|| RegistryError::invalid("custodied_cutoff"))?;
        if let Some(nonce) = self
            .source
            .parent()
            .try_submit(command, until)
            .map_err(|cause| RegistryError::new("custodied_command", cause))?
        {
            self.destination
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("custodied_destination"))?
                .bind_nonce(nonce)?;
            self.nonce = Some(nonce);
            self.queued = None;
        }
        Ok(())
    }
    /// Move original native preparation before any root creation.
    fn prepare(&mut self, context: &JobContext) -> Result<(), RegistryError> {
        if !self.source.parent().established() || !self.source.parent().transaction_available() {
            return Ok(());
        }
        self.check()?;
        let prepared = self
            .source
            .preparation(context)
            .map_err(|cause| RegistryError::new("custodied_claim", cause))?;
        let until = self
            .client
            .encode_lifecycle_deadline(
                context.deadline().ok_or_else(|| RegistryError::invalid("custodied_cutoff"))?,
            )
            .map_err(|cause| RegistryError::new("custodied_counter", cause))?;
        let input = &mut self
            .input
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custodied_input"))?
            .value;
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custodied_family"))?;
        family.adopt_output(input, self.ticket.binding)?;
        family.prepare_inheritance()?;
        self.shared.port.prepare_job(family)?;
        let target = self
            .destination
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custodied_destination"))?;
        target.open()?;
        let job =
            target.duplicate(native_process::handle(family.job, "custodied_prepared_job")?)?;
        self.queued = Some(Command::Prepare {
            resource: self.source.resource(),
            digest: prepared.digest,
            job,
            until_ticks: until.ticks(),
            frequency: until.frequency(),
            record: prepared.record,
            claim_identity: prepared.writer,
            registry_process: self.ticket.process.id.get(),
        });
        self.phase = Phase::WaitingPrepared;
        self.send(context)
    }
    /// Create the actual root suspended only after the exact prepared acknowledgement.
    fn create_root(&mut self, context: &JobContext) -> Result<(), RegistryError> {
        self.check()?;
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custodied_family"))?;
        let input = &mut self
            .input
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custodied_input"))?
            .value;
        let root = self.shared.port.create(family, input)?;
        self.facts = Some(CreationFacts { root, resumed: false });
        let duplicate = self.shared.port.duplicate(family, 0)?;
        let permit = lock(&self.shared.permit)
            .take()
            .ok_or_else(|| RegistryError::invalid("custodied_root_watch"))?;
        let watch = submission::bind(permit, duplicate);
        lock(&self.shared.native).watch = watch;
        let process = self
            .destination
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custodied_destination"))?
            .duplicate(native_process::handle(family.member(0)?.handle, "custodied_root")?)?;
        self.queued = Some(Command::Root {
            resource: self.source.resource(),
            process,
            pid: root.pid,
            birth: root.birth,
            registry_process: self.ticket.process.id.get(),
        });
        self.phase = Phase::WaitingRoot;
        self.send(context)
    }
    /// Commit exactly one original resume after the correlated actual root adoption.
    fn resume(&mut self) -> Result<(), RegistryError> {
        self.check()?;
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custodied_family"))?;
        family.validate_output_for_resume()?;
        if self
            .shared
            .admission
            .compare_exchange(OPEN, RESUME_COMMITTED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RegistryError::invalid("custodied_resume_closed"));
        }
        let resumed = self.shared.port.resume(family);
        if let Some(facts) = &mut self.facts {
            facts.resumed = family.resumed;
        }
        resumed?;
        self.phase = Phase::Started;
        Ok(())
    }
}
impl CustodiedProcessDriver for Driver {
    fn initialize(
        &mut self,
        _: &JobContext,
        memory: &CustodiedProcessMemory,
        _: CustodiedProcessWake,
    ) -> Result<(), NativeObservationError> {
        if self.memory.as_ref().is_none_or(|actual| !actual.same_allocation(memory)) {
            return Err(native(RegistryError::invalid("custodied_memory_changed")));
        }
        self.check().map_err(native)
    }
    fn progress(
        &mut self,
        context: &JobContext,
        _: Moment,
    ) -> Result<CustodiedProcessProgress, NativeObservationError> {
        let result = (|| {
            self.source
                .parent()
                .consume(
                    std::num::NonZeroUsize::new(32)
                        .ok_or_else(|| RegistryError::invalid("custodied_bound"))?,
                )
                .map_err(|cause| RegistryError::new("custodied_observations", cause))?;
            if matches!(self.phase, Phase::Prepared) {
                self.prepare(context)?;
            }
            self.send(context)?;
            if let Some(nonce) = self.nonce
                && let Some(ack) = self.source.parent().take_acknowledgement_for(nonce)
            {
                let expected = match self.phase {
                    Phase::WaitingPrepared => Receipt::Prepared(self.source.resource()),
                    Phase::WaitingRoot => Receipt::Root(self.source.resource()),
                    _ => return Err(RegistryError::invalid("custodied_ack_phase")),
                };
                if ack.receipt() != expected {
                    return Err(RegistryError::invalid("custodied_ack_receipt"));
                }
                self.destination
                    .as_mut()
                    .ok_or_else(|| RegistryError::invalid("custodied_destination"))?
                    .acknowledge(&ack)?;
                self.nonce = None;
                match self.phase {
                    Phase::WaitingPrepared => self.create_root(context)?,
                    Phase::WaitingRoot => self.resume()?,
                    _ => {}
                }
            }
            Ok(if matches!(self.phase, Phase::Started) {
                CustodiedProcessProgress::Started
            } else {
                CustodiedProcessProgress::Pending
            })
        })();
        result.map_err(native)
    }
    fn retire(&mut self, _: &JobContext, _: Moment) -> Result<bool, NativeObservationError> {
        let memory = self
            .memory
            .as_ref()
            .ok_or_else(|| native(RegistryError::invalid("custodied_memory")))?;
        let proof = memory
            .begin_retirement()
            .map_err(|cause| native(RegistryError::new("custodied_retirement", cause)))?;
        proof.check().map_err(|cause| native(RegistryError::new("custodied_retirement", cause)))?;
        let clean = self
            .destination
            .as_mut()
            .map(|target| target.close_acknowledged(&proof))
            .transpose()
            .map_err(native)?
            .unwrap_or(true);
        let mut state = lock(&self.shared.native);
        if state.family.is_some() {
            return Err(native(RegistryError::invalid("custodied_transfer_owner")));
        }
        let mut family = self.family.take();
        if let Some(family) = &mut family {
            family.creation_transferred();
        }
        state.family = family;
        state.creation = self.facts;
        if !clean {
            state.custody_destination = self.destination.take();
        }
        drop(state);
        drop(self.input.take());
        Ok(true)
    }
}
/// Preserve original typed/native causes under the central worker's outside claim.
fn native(cause: RegistryError) -> NativeObservationError {
    NativeObservationError::new("custodied_native_creation", cause)
}
