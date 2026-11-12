// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pre-admit a genuine recovery carrier and retain its held terminal in the real registry.
use super::*;
use sugarsubstitute_contracts::incident::DurableAcknowledgement;
use sugarsubstitute_execution::RegisteredRecoveryGrant;
impl WindowsProcessRegistry {
    /// Retain the existing prepared family/watch and reserve its original finite native worker.
    /// Input is unchanged on rejection; no native factory can execute until the exact ACK.
    pub(crate) fn reserve_recovery_profile_creation(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        grant: &RegisteredRecoveryGrant,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<LaunchTicket, RegistryAdmissionError> {
        grant.check().map_err(RegistryAdmissionError::Execution)?;
        if RegisteredRecoveryGrant::control_storage_bytes()
            + size_of::<super::super::creation_job::HeldRecoveryCreation>()
            > 4096
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        submission::validate_blocking(spec)?;
        let original = input.as_ref().ok_or(RegistryAdmissionError::InvalidSpecification)?;
        let prepared = original
            .profile_custody
            .as_ref()
            .ok_or(RegistryAdmissionError::InvalidSpecification)?;
        let source = &prepared.value;
        if !self.launch_open || grant.key().run().owner != self.owner
            || source.key.run() != grant.key().run()
            || source.recovery_grant.as_ref().is_none_or(|actual| actual.key() != grant.key())
            || !source.identity.same_as(&self.identity)
            || source.published_ack.as_ref().is_none_or(|ack| ack.receipt() != sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::ProfilePublished(source.key))
            || spec.deadline != Some(grant.original_budget().until())
            || spec.reservation_bytes < original.required_reservation_bytes()
            || original.managed_custody.is_some() || original.backend_custody.is_some()
            || original.purpose.is_some() || original.lifecycle.is_some() || original.fixture_events.is_some()
            || original.recovery_carrier.as_ref().is_none_or(|carrier| carrier.grant.key() != grant.key() || carrier.enrollment.as_ref().is_none_or(|enrollment| !carrier.bootstrap.matches_enrollment(enrollment)))
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let index = self
            .records
            .iter()
            .position(|record| {
                record.as_ref().is_some_and(|record| {
                    record.ticket.process.id.get() == source.key.registry_process()
                        && record.ticket.binding == FamilyBinding::Desktop(grant.key().run())
                })
            })
            .ok_or(RegistryAdmissionError::Stale)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if record.started
            || record.settled
            || record.creation.is_some()
            || record.shared.admission.load(Ordering::Acquire) != OPEN
            || lock(&record.shared.native).pending_input.is_some()
            || record.root_spec.1 >= spec.id
        {
            return Err(RegistryAdmissionError::Closed);
        }
        let parent = source.parent.clone();
        let ticket = record.ticket;
        let client = execution.clone();
        let shared = record.shared.clone();
        lock(&shared.native).pending_input = input.take();
        let mut retained = Some(shared.clone());
        let result = execution.reserve_recovery_custody_creation(
            scope,
            spec,
            parent.storage(),
            grant,
            &mut retained,
            move |shared, context, memory| {
                let input = lock(&shared.native).pending_input.take().ok_or_else(|| {
                    NativeObservationError::new(
                        "recovery_profile_input",
                        RegistryError::invalid("recovery_profile_input"),
                    )
                })?;
                custodied_creation::Driver::construct(
                    shared, input, context, memory, client, ticket,
                )
            },
        );
        match result {
            Ok(permit) => {
                record.creation = Some(CreationJob::HeldRecovery {
                    owner: Box::new(super::super::creation_job::HeldRecoveryCreation {
                        permit: Some(permit),
                        scope: scope.id(),
                        spec,
                    }),
                    _grant: grant.clone().into_custody(),
                })
            }
            Err(cause) => {
                *input = lock(&record.shared.native).pending_input.take();
                return Err(RegistryAdmissionError::Execution(cause));
            }
        }
        Ok(ticket)
    }
    /// Observe genuine held queue/input/profile/watch custody without activating or minting authority.
    pub(crate) fn recovery_profile_creation_ready(
        &self,
        ticket: LaunchTicket,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<bool, RegistryAdmissionError> {
        grant.check().map_err(RegistryAdmissionError::Execution)?;
        let index = self.index(ticket.process)?;
        let record = self.records[index].as_ref().ok_or(RegistryAdmissionError::Stale)?;
        if record.ticket != ticket || ticket.binding != FamilyBinding::Desktop(grant.key().run()) {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        let held = matches!(&record.creation,Some(CreationJob::HeldRecovery{owner,..}) if owner.permit.as_ref().is_some_and(|permit|permit.key()==grant.key())&&owner.spec.deadline==Some(grant.original_budget().until()));
        let state = lock(&record.shared.native);
        let input = state.pending_input.as_ref();
        Ok(held&&self.launch_open&&!record.settled&&!record.started&&record.recovery_watch_issued
            &&record.shared.admission.load(Ordering::Acquire)==OPEN&&lock(&record.shared.permit).is_some()
            &&input.is_some_and(|input|input.profile_custody.as_ref().is_some_and(|profile|profile.value.recovery_grant.as_ref().is_some_and(|actual|actual.key()==grant.key())&&profile.value.family.as_ref().is_some_and(|family|family.job.is_some())&&profile.value.published_ack.as_ref().is_some_and(|ack|ack.receipt()==sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::ProfilePublished(profile.value.key)))
                &&input.recovery_carrier.as_ref().is_some_and(|carrier|carrier.grant.key()==grant.key()&&carrier.enrollment.as_ref().is_some_and(|enrollment|carrier.bootstrap.matches_enrollment(enrollment)))))
    }
    /// Release only this exact actual held job; failed activation restores the identical owner.
    pub(crate) fn begin_recovery_profile_creation(
        &mut self,
        ticket: LaunchTicket,
        grant: &RegisteredRecoveryGrant,
        acknowledgement: DurableAcknowledgement,
    ) -> Result<(), RegistryAdmissionError> {
        grant.check().map_err(RegistryAdmissionError::Execution)?;
        if ticket.binding != FamilyBinding::Desktop(grant.key().run()) {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        let index = self.index(ticket.process)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if record.ticket != ticket || record.shared.admission.load(Ordering::Acquire) != OPEN {
            return Err(RegistryAdmissionError::Closed);
        }
        let job = record.creation.take().ok_or(RegistryAdmissionError::Stale)?;
        let CreationJob::HeldRecovery { mut owner, _grant } = job else {
            record.creation = Some(job);
            return Err(RegistryAdmissionError::InvalidSpecification);
        };
        let Some(permit) = owner.permit.take() else {
            record.creation = Some(CreationJob::HeldRecovery { owner, _grant });
            return Err(RegistryAdmissionError::InvalidSpecification);
        };
        if permit.key() != grant.key() {
            owner.permit = Some(permit);
            record.creation = Some(CreationJob::HeldRecovery { owner, _grant });
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        match permit.begin(acknowledgement) {
            Ok(job) => {
                drop(owner);
                record.creation = Some(CreationJob::Custodied(job));
            }
            Err(rejected) => {
                owner.permit = Some(rejected.permit);
                record.creation = Some(CreationJob::HeldRecovery { owner, _grant });
                return Err(RegistryAdmissionError::Execution(rejected.error));
            }
        }
        Ok(())
    }
}
