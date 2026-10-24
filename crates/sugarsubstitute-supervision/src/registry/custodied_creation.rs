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
use sugarsubstitute_execution::process_creation::{
    ProcessCreationDriver, ProcessCreationMemory, ProcessCreationProgress, ProcessCreationWake,
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
    memory: Option<ProcessCreationMemory>,
    phase: Phase,
    client: ExecutionClient,
    ticket: LaunchTicket,
    source: Source,
    _profile_metadata: Option<std::sync::Arc<sugarsubstitute_contracts::execution::MemoryClaim>>,
}
enum Source {
    Managed(ManagedCustodySource),
    Profile {
        parent: crate::BackendCustodyParent,
        key: sugarsubstitute_contracts::backend_custody::ProfileCustodyKey,
    },
}
impl Source {
    fn parent(&self) -> &crate::BackendCustodyParent {
        match self {
            Self::Managed(source) => source.parent(),
            Self::Profile { parent, .. } => parent,
        }
    }
    fn root_receipt(&self) -> Receipt {
        match self {
            Self::Managed(source) => Receipt::Root(source.resource()),
            Self::Profile { key, .. } => Receipt::ProfileRoot(*key),
        }
    }
}
impl Driver {
    /// Claim real input capacities before constructing the actual retained driver.
    pub(super) fn construct(
        shared: SharedOwner,
        mut input: WindowsLaunchInput,
        context: &JobContext,
        memory: &ProcessCreationMemory,
        client: ExecutionClient,
        ticket: LaunchTicket,
    ) -> Result<Box<dyn ProcessCreationDriver>, NativeObservationError> {
        if Family::storage_bytes() + size_of::<Self>() > memory.bytes() as usize {
            return Err(native(RegistryError::invalid("custodied_family_capacity")));
        }
        let profile = input.profile_custody.take();
        let mut profile_metadata = None;
        let (source, family, destination, phase) = if let Some(mut profile) = profile {
            profile_metadata = Some(profile.memory);
            let profile = &mut profile.value;
            if let Some(grant) = &profile.recovery_grant {
                grant.check_context(context).map_err(|cause| {
                    native(RegistryError::new("custodied_recovery_context", cause))
                })?;
                if input
                    .recovery_carrier
                    .as_ref()
                    .is_none_or(|carrier| carrier.grant.key() != grant.key())
                {
                    return Err(native(RegistryError::invalid("custodied_recovery_carrier")));
                }
            } else if input.recovery_carrier.is_some() {
                return Err(native(RegistryError::invalid("custodied_ordinary_carrier")));
            }
            if ticket.binding != FamilyBinding::Desktop(profile.key.run())
                || context
                    .deadline()
                    .is_none_or(|until| until > profile.until || context.now() >= profile.until)
                || profile
                    .published_ack
                    .as_ref()
                    .is_none_or(|ack| ack.receipt() != Receipt::ProfilePublished(profile.key))
                || !profile.identity.same_as(shared.identity())
            {
                return Err(native(RegistryError::invalid("custodied_profile_source")));
            }
            (
                Source::Profile { parent: profile.parent.clone(), key: profile.key },
                profile
                    .family
                    .take()
                    .ok_or_else(|| native(RegistryError::invalid("custodied_profile_family")))?,
                profile.destination.take().ok_or_else(|| {
                    native(RegistryError::invalid("custodied_profile_destination"))
                })?,
                Phase::WaitingPrepared,
            )
        } else {
            let source = input
                .managed_custody
                .take()
                .ok_or_else(|| native(RegistryError::invalid("custodied_source")))?;
            if ticket.binding != FamilyBinding::Managed(source.resource()) {
                return Err(native(RegistryError::invalid("custodied_source_association")));
            }
            let family = Family::custodied(memory, size_of::<Self>()).map_err(native)?;
            let destination = CustodyDestination::new(source.parent());
            (Source::Managed(source), family, destination, Phase::Prepared)
        };
        if !source.parent().matches_execution(&client) {
            return Err(native(RegistryError::invalid("custodied_source_execution")));
        }
        let transient =
            input.required_reservation_bytes() - crate::registry_input::FAMILY_STORAGE_BYTES;
        let claim = context
            .memory()
            .claim(transient)
            .map_err(|cause| native(RegistryError::new("custodied_input_memory", cause)))?;
        Ok(Box::new(Self {
            shared,
            input: Some(Input { value: input, _memory: claim }),
            family: Some(family),
            destination: Some(destination),
            queued: None,
            nonce: None,
            facts: None,
            memory: Some(memory.clone()),
            phase,
            client,
            ticket,
            source,
            _profile_metadata: profile_metadata,
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
        let Source::Managed(source) = &self.source else {
            return Err(RegistryError::invalid("custodied_profile_prepare_phase"));
        };
        let prepared = source
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
            resource: source.resource(),
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
        if matches!(self.source, Source::Profile { .. }) {
            family.adopt_recovery_carrier(input)?;
            family.adopt_lifecycle(input)?;
            family.adopt_output(input, self.ticket.binding)?;
            family.prepare_inheritance()?;
        }
        let root = self.shared.port.create(family, input)?;
        if let Source::Profile { key, .. } = &self.source {
            tracing::info!(operation="profile_root_suspended",profile=?key,pid=root.pid,birth=root.birth,
                "Actual root created suspended in the original prepared desktop Job");
        }
        self.facts = Some(CreationFacts { root, resumed: false });
        let duplicate = self.shared.port.duplicate(family, 0)?;
        let permit = lock(&self.shared.permit)
            .take()
            .ok_or_else(|| RegistryError::invalid("custodied_root_watch"))?;
        let watch = creation::bind(permit, duplicate);
        lock(&self.shared.native).watch = watch;
        if let Source::Profile { key, .. } = &self.source {
            self.shared.port.enroll_desktop(family, key.run(), self.ticket.process)?;
        }
        let process = self
            .destination
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custodied_destination"))?
            .duplicate(native_process::handle(family.member(0)?.handle, "custodied_root")?)?;
        self.queued = Some(match &self.source {
            Source::Managed(source) => Command::Root {
                resource: source.resource(),
                process,
                pid: root.pid,
                birth: root.birth,
                registry_process: self.ticket.process.id.get(),
            },
            Source::Profile { key, .. } => {
                Command::ProfileRoot { profile: *key, process, pid: root.pid, birth: root.birth }
            }
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
impl ProcessCreationDriver for Driver {
    fn initialize(
        &mut self,
        _: &JobContext,
        memory: &ProcessCreationMemory,
        _: ProcessCreationWake,
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
    ) -> Result<ProcessCreationProgress, NativeObservationError> {
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
            if matches!(self.source, Source::Profile { .. })
                && matches!(self.phase, Phase::WaitingPrepared)
                && self.nonce.is_none()
            {
                self.create_root(context)?;
            }
            self.send(context)?;
            if let Some(nonce) = self.nonce
                && let Some(ack) = self.source.parent().take_acknowledgement_for(nonce)
            {
                let expected = match self.phase {
                    Phase::WaitingPrepared => match &self.source {
                        Source::Managed(source) => Receipt::Prepared(source.resource()),
                        Source::Profile { .. } => {
                            return Err(RegistryError::invalid("custodied_profile_ack_phase"));
                        }
                    },
                    Phase::WaitingRoot => self.source.root_receipt(),
                    _ => return Err(RegistryError::invalid("custodied_ack_phase")),
                };
                if ack.receipt() != expected {
                    return Err(RegistryError::invalid("custodied_ack_receipt"));
                }
                self.destination
                    .as_mut()
                    .ok_or_else(|| RegistryError::invalid("custodied_destination"))?
                    .acknowledge(&ack)?;
                if let Source::Profile { key, .. } = &self.source {
                    tracing::info!(operation="profile_root_acknowledged",profile=?key,nonce,
                        "Actual helper adopted the suspended root before resume commitment");
                }
                self.nonce = None;
                match self.phase {
                    Phase::WaitingPrepared => self.create_root(context)?,
                    Phase::WaitingRoot => self.resume()?,
                    _ => {}
                }
            }
            Ok(if matches!(self.phase, Phase::Started) {
                ProcessCreationProgress::Started
            } else {
                ProcessCreationProgress::Pending
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
