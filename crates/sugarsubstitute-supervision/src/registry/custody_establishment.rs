// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Establish the actual independent helper through finite owned native preparation.
use super::*;
use crate::setup_storage::{
    RunningApplication, TRIAL_WITNESS_RESERVATION_BYTES, TrialWitnessTransfer,
};
use crate::{BackendCustodyParent, native_process::custody_transfer::CustodyDestination};
use std::os::windows::io::AsRawHandle;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyCommand, BackendCustodyReceipt},
    execution::{JobOutcome, Moment},
};
pub(super) struct Establishment {
    destination: CustodyDestination,
    command: Option<BackendCustodyCommand>,
    until: Moment,
    nonce: Option<u64>,
    complete: bool,
    trial: Option<TrialEstablishment>,
}
struct TrialEstablishment {
    transfer: TrialWitnessTransfer,
    destination: CustodyDestination,
}
pub(super) struct EstablishmentResult;
impl Establishment {
    pub(super) fn close_after_exit(&mut self) -> Result<(), RegistryError> {
        if self.destination.close_after_exit()?
            && self
                .trial
                .as_mut()
                .map(|trial| trial.destination.close_after_exit())
                .transpose()?
                .unwrap_or(true)
        {
            Ok(())
        } else {
            Err(RegistryError::invalid("custody_establishment_target_live"))
        }
    }
}
impl WindowsProcessRegistry {
    /// Account for independent protected snapshot work only when this coordinator owns a trial.
    pub fn backend_custody_establishment_bytes(application: Option<&RunningApplication>) -> u64 {
        crate::registry_input::FAMILY_STORAGE_BYTES
            + if matches!(application, Some(RunningApplication::Trial(_))) {
                TRIAL_WITNESS_RESERVATION_BYTES
            } else {
                0
            }
    }
    /// Prepare exact parent/self-Job duplicates after authenticating this actual helper.
    /// The original finite specification and helper quota are preserved.
    /// # Errors
    /// Rejects foreign/pending/closed helpers and preserves native or central admission causes.
    pub fn submit_backend_custody_establishment(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        parent: &BackendCustodyParent,
        ticket: LaunchTicket,
        application: Option<Arc<RunningApplication>>,
    ) -> Result<(), RegistryAdmissionError> {
        validate_blocking(spec)?;
        let until = spec.deadline.ok_or(RegistryAdmissionError::InvalidSpecification)?;
        if spec.reservation_bytes
            != Self::backend_custody_establishment_bytes(application.as_deref())
            || ticket.binding != FamilyBinding::BackendCustody(parent.key())
            || !parent.matches_execution(execution)
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let at = self.index(ticket.process)?;
        let record = self.records[at].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if !record.started
            || record.creation.is_some()
            || record.establishment_job.is_some()
            || record.establishment_completion.is_some()
            || !parent.authenticated()
            || record.shared.admission.load(Ordering::Acquire) == CLOSED
        {
            return Err(RegistryAdmissionError::Busy);
        }
        let shared = record.shared.clone();
        let original = parent.clone();
        let client = execution.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(crate::registry_input::FAMILY_STORAGE_BYTES)
                    .map_err(|cause| RegistryError::new("custody_establishment_memory", cause))?;
                original
                    .check()
                    .map_err(|cause| RegistryError::new("custody_establishment_source", cause))?;
                let (family, previous) = {
                    let mut state = lock(&shared.native);
                    (state.family.take(), state.establishment.take())
                };
                let mut loan = EstablishmentLoan { shared: shared.clone(), family, data: previous };
                if loan.data.is_some() {
                    return Err(RegistryError::invalid("custody_establishment_repeated"));
                }
                let family = loan
                    .family
                    .as_ref()
                    .ok_or_else(|| RegistryError::invalid("custody_establishment_family"))?;
                let job = native_process::handle(family.job, "custody_establishment_job")?;
                let peer = original
                    .check()
                    .map_err(|cause| RegistryError::new("custody_establishment_source", cause))?;
                if peer.process() != Some(ticket.process)
                    || family.root
                        != Some(Identity {
                            pid: peer.process_id(),
                            birth: peer.creation_time_100ns(),
                        })
                {
                    return Err(RegistryError::invalid("custody_establishment_helper"));
                }
                loan.data = Some(Box::new(Establishment {
                    destination: CustodyDestination::new(&original),
                    command: None,
                    until,
                    nonce: None,
                    complete: false,
                    trial: None,
                }));
                let data = loan
                    .data
                    .as_mut()
                    .ok_or_else(|| RegistryError::invalid("custody_establishment_data"))?;
                data.destination.open()?;
                let (parent_handle, identity) = data.destination.duplicate_current_parent()?;
                let containment = data.destination.duplicate(job)?;
                let trial =
                    if let Some(RunningApplication::Trial(application)) = application.as_deref() {
                        data.trial = Some(TrialEstablishment {
                            transfer: application.prepare_witness(&context).map_err(|cause| {
                                RegistryError::new("custody_trial_snapshot", cause)
                            })?,
                            destination: CustodyDestination::new(&original),
                        });
                        let trial = data
                            .trial
                            .as_mut()
                            .ok_or_else(|| RegistryError::invalid("custody_trial_owner"))?;
                        trial.destination.open()?;
                        let handles = trial.transfer.handles();
                        let run = trial.destination.duplicate(
                            windows::Win32::Foundation::HANDLE(handles[0].as_raw_handle()),
                        )?;
                        let snapshot = trial.destination.duplicate(
                            windows::Win32::Foundation::HANDLE(handles[1].as_raw_handle()),
                        )?;
                        let counter = client
                            .encode_lifecycle_deadline(until)
                            .map_err(|cause| RegistryError::new("custody_trial_counter", cause))?;
                        Some(sugarsubstitute_contracts::backend_custody::InstallationTrialCustody {
                            run,
                            snapshot,
                            until_ticks: counter.ticks(),
                            frequency: counter.frequency(),
                        })
                    } else {
                        None
                    };
                data.command = Some(BackendCustodyCommand::Establish {
                    parent: parent_handle,
                    pid: identity.pid,
                    birth: identity.birth,
                    containment,
                    trial,
                });
                Ok(EstablishmentResult)
            })
            .map_err(RegistryAdmissionError::Execution)?;
        record.establishment_job = Some(job);
        Ok(())
    }
    /// Consume the real establishment outcome and its original correlated receipt.
    /// This advances bounded transport metadata; it spawns no worker and returns promptly.
    /// # Errors
    /// Preserves the original finite/native failure and rejects changed helper ownership.
    pub fn advance_backend_custody_establishment(
        &mut self,
        parent: &BackendCustodyParent,
        ticket: LaunchTicket,
    ) -> Result<bool, RegistryError> {
        let at = self
            .index(ticket.process)
            .map_err(|cause| RegistryError::new("custody_establishment_ticket", cause))?;
        let record = self.records[at]
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_establishment_record"))?;
        if ticket.binding != FamilyBinding::BackendCustody(parent.key()) {
            return Err(RegistryError::invalid("custody_establishment_binding"));
        }
        let Some(completion) = record.establishment_completion.as_ref() else {
            return Ok(false);
        };
        match &completion.outcome {
            JobOutcome::Success(_) => {}
            JobOutcome::Failure(cause) => return Err(cause.clone()),
            _ => return Err(RegistryError::invalid("custody_establishment_terminal")),
        }
        let data = lock(&record.shared.native)
            .establishment
            .take()
            .ok_or_else(|| RegistryError::invalid("custody_establishment_missing"))?;
        let mut loan = EstablishmentDataLoan { data: Some(data), shared: record.shared.clone() };
        let data = loan
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_establishment_missing"))?;
        if data.complete {
            return Ok(true);
        }
        parent
            .consume(
                std::num::NonZeroUsize::new(32)
                    .ok_or_else(|| RegistryError::invalid("custody_establishment_bound"))?,
            )
            .map_err(|cause| RegistryError::new("custody_establishment_observations", cause))?;
        if data.nonce.is_none() {
            data.nonce = parent
                .try_submit(
                    data.command
                        .ok_or_else(|| RegistryError::invalid("custody_establishment_command"))?,
                    data.until,
                )
                .map_err(|cause| RegistryError::new("custody_establishment_command", cause))?;
            if let Some(nonce) = data.nonce {
                data.destination.bind_nonce(nonce)?;
                if let Some(trial) = &mut data.trial {
                    trial.destination.bind_nonce(nonce)?;
                }
            }
        }
        if let Some(nonce) = data.nonce
            && let Some(ack) = parent.take_acknowledgement_for(nonce)
        {
            if ack.receipt() != BackendCustodyReceipt::Established {
                return Err(RegistryError::invalid("custody_establishment_receipt"));
            }
            data.destination.acknowledge_metadata(&ack)?;
            if let Some(trial) = &mut data.trial {
                trial.destination.acknowledge_metadata(&ack)?;
            }
            data.complete = true;
        }
        Ok(data.complete)
    }
}
/// Restore every original family/output before reporting failure or unexpected unwind.
struct EstablishmentLoan {
    family: Option<native_process::FamilyStorage>,
    data: Option<Box<Establishment>>,
    shared: SharedOwner,
}
impl Drop for EstablishmentLoan {
    fn drop(&mut self) {
        let mut state = lock(&self.shared.native);
        state.family = self.family.take();
        state.establishment = self.data.take();
    }
}
struct EstablishmentDataLoan {
    data: Option<Box<Establishment>>,
    shared: SharedOwner,
}
impl Drop for EstablishmentDataLoan {
    fn drop(&mut self) {
        lock(&self.shared.native).establishment = self.data.take();
    }
}
