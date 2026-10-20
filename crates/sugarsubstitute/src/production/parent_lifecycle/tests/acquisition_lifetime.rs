// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Churn real replay/Parent owners; fake only backend/write facts, never IPC/native authority.
use super::super::acquisition::Acquisition;
use super::*;
use sugarsubstitute_application::supervision::MainAuthorizationObservation;
use sugarsubstitute_contracts::{
    configuration::ProfileId,
    lifecycle::{
        ClientBinding, LeaseId, LeaseKey, LifecycleRejection, RequestOutcome, ResourceGeneration,
        ResourceId, ResourceKey,
    },
};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

fn moment(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}
fn binding() -> Result<ClientBinding> {
    Ok(ClientBinding { run: identity()?.run, connection: ConnectionId::new(1)? })
}
fn lease() -> Result<LeaseKey> {
    Ok(LeaseKey {
        run: identity()?.run,
        connection: binding()?.connection,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: identity()?.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    })
}
fn actor() -> Result<ParentLifecycle> {
    let id = identity()?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    Ok(ParentLifecycle::new(
        id.run,
        id.launch,
        ProfileId::new("Acquire_1".into())?,
        binding()?.connection,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?),
    )?)
}
fn acquisition_header() -> Result<WireHeader> {
    Ok(WireHeader { deadline_qpc: 180_000_000_000, ..header(3)? })
}
fn granted() -> Result<ParentLifecycle> {
    let mut parent = actor()?;
    let h = acquisition_header()?;
    let lease = lease()?;
    let id = identity()?;
    let (index, _) = parent.requests.admit(h, b"original acquisition boundary bytes")?;
    parent.requests.get_mut(index)?.identity = Some(id);
    parent.requests.get_mut(index)?.until = Some(moment(180));
    let mut acquisition = Acquisition::new(binding()?, h, id, moment(180))?;
    for outcome in [RequestOutcome::Pending(lease), RequestOutcome::Granted(lease)] {
        parent.requests.observe_backend(index, outcome)?;
        acquisition.observe_backend(h, outcome)?;
    }
    let reply = Reply::Granted { lease, identity: id };
    acquisition.reply(h, reply)?;
    // The native writer completion is an external boundary fact. This fixture creates
    // no peer, write admission, frame loan, readiness proof or backend release witness.
    complete_acquisition_write(&mut acquisition, h)?;
    parent.requests.get_mut(index)?.reply = Some(reply);
    parent.requests.get_mut(index)?.sent = true;
    parent.acquisition = Some(acquisition);
    parent.lease = Some(lease);
    parent.main.begin(id, lease, moment(30))?;
    Ok(parent)
}
fn churn(parent: &mut ParentLifecycle) -> Result<()> {
    for id in 4..=4000 {
        let h = header(id)?;
        let mut bytes = [0; 512];
        let n = FrameCodec::encode(
            &WireMessage {
                header: h,
                payload: WirePayload::OutputPull {
                    lease: super::super::replies::reference(lease()?),
                    acknowledged_delivery: None,
                },
            },
            &mut bytes,
        )?;
        let (index, fresh) = parent.requests.admit(h, &bytes[..n])?;
        assert!(fresh);
        parent.requests.get_mut(index)?.reply = Some(Reply::OutputIdle(lease()?));
        parent.requests.get_mut(index)?.sent = true;
    }
    assert!(parent.requests.find(RequestId::new(3)?).is_none());
    assert_eq!(
        parent
            .requests
            .admit(acquisition_header()?, b"original acquisition boundary bytes")
            .map_err(|cause| cause.code()),
        Err(ParentError::StaleRequest)
    );
    Ok(())
}
fn retire(parent: &mut ParentLifecycle, id: MainIdentity) -> Result<()> {
    parent.main.retire(id)?;
    parent.main.accept(
        binding()?,
        RequestId::new(4001)?,
        MainAuthorizationObservation::MainRetired { identity: id },
        moment(220),
    )?;
    parent.main.confirm_retired(id)?;
    Ok(())
}
fn prepare(
    parent: &mut ParentLifecycle,
    id: MainIdentity,
    acquisition: RequestId,
) -> Result<sugarsubstitute_contracts::lifecycle::ShutdownBudget> {
    let (index, _) = parent.requests.admit(header(5000)?, b"original settlement boundary bytes")?;
    parent.requests.get_mut(index)?.until = Some(moment(235));
    parent
        .prepare_attempt_cleanup(index, id, acquisition, moment(235), moment(221), false)
        .map_err(|cause| cause as Box<dyn Error>)
}

#[test]
fn current_acquisition_survives_thousands_of_replay_evictions_and_selects_exact_cleanup()
-> Result<()> {
    let mut parent = granted()?;
    churn(&mut parent)?;
    retire(&mut parent, identity()?)?;
    let budget = prepare(&mut parent, identity()?, RequestId::new(3)?)?;
    assert_eq!(budget.started, moment(220));
    assert_eq!(budget.settle_until, moment(235));
    let cleanup = parent.attempt_cleanup.as_ref().ok_or("Cleanup missing")?;
    assert_eq!(cleanup.origin, acquisition_header()?);
    assert_eq!(cleanup.lease, Some(lease()?));
    assert_eq!(cleanup.resource, Some(lease()?.resource));
    assert_eq!(cleanup.identity, identity()?);
    assert_eq!(parent.acquisition.as_ref().ok_or("Acquisition missing")?.original, moment(180));
    assert!(!parent.native_retired());
    assert!(parent.requests.get(cleanup.index)?.reply.is_none());
    assert!(!parent.requests.get(cleanup.index)?.sent);
    Ok(())
}

#[test]
fn churn_does_not_accept_foreign_prior_request_identity_or_current_lease() -> Result<()> {
    for fault in 0..4 {
        let mut parent = granted()?;
        churn(&mut parent)?;
        retire(&mut parent, identity()?)?;
        let mut id = identity()?;
        let request = if fault == 0 { RequestId::new(2)? } else { RequestId::new(3)? };
        match fault {
            1 => id.launch = LaunchId::new(2)?,
            2 => id.run.instance = InstanceId::new(2)?,
            3 => parent.lease = Some(LeaseKey { id: LeaseId::new(2)?, ..lease()? }),
            _ => {}
        }
        assert!(prepare(&mut parent, id, request).is_err());
        assert!(parent.attempt_cleanup.is_none());
        assert_eq!(parent.acquisition.as_ref().ok_or("Origin lost")?.header, acquisition_header()?);
    }
    Ok(())
}

#[test]
fn cleanup_requires_original_checked_write_and_exact_counter_binding() -> Result<()> {
    let mut retained =
        Acquisition::new(binding()?, acquisition_header()?, identity()?, moment(180))?;
    retained.observe_backend(acquisition_header()?, RequestOutcome::Granted(lease()?))?;
    retained
        .reply(acquisition_header()?, Reply::Granted { lease: lease()?, identity: identity()? })?;
    assert!(
        retained
            .settlement_lease(
                binding()?,
                identity()?,
                RequestId::new(3)?,
                Some(lease()?),
                Some(identity()?)
            )
            .is_err()
    );
    let wrong = WireHeader { deadline_qpc: 180_000_000_001, ..acquisition_header()? };
    assert_eq!(
        retained.observe_write(wrong, super::super::acquisition::WriteObservation::Written),
        Err(ParentError::ForeignPeer)
    );
    let foreign = ClientBinding { connection: ConnectionId::new(2)?, ..binding()? };
    complete_acquisition_write(&mut retained, acquisition_header()?)?;
    assert!(
        retained
            .settlement_lease(
                foreign,
                identity()?,
                RequestId::new(3)?,
                Some(lease()?),
                Some(identity()?)
            )
            .is_err()
    );
    assert_eq!(
        retained.settlement_lease(
            binding()?,
            identity()?,
            RequestId::new(3)?,
            Some(lease()?),
            Some(identity()?)
        )?,
        Some(lease()?)
    );
    assert_eq!(retained.header, acquisition_header()?);
    assert_eq!(retained.original, moment(180));
    let next_identity =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..identity()? };
    let next_header = WireHeader {
        request: Some(RequestId::new(5001)?),
        deadline_qpc: 400_000_000_000,
        ..acquisition_header()?
    };
    let next_lease = LeaseKey {
        id: LeaseId::new(2)?,
        resource: ResourceKey { generation: ResourceGeneration::new(2)?, ..lease()?.resource },
        ..lease()?
    };
    let mut successor = Acquisition::new(binding()?, next_header, next_identity, moment(400))?;
    successor.observe_backend(next_header, RequestOutcome::Granted(next_lease))?;
    successor.reply(next_header, Reply::Granted { lease: next_lease, identity: next_identity })?;
    complete_acquisition_write(&mut successor, next_header)?;
    assert_eq!(
        successor.settlement_lease(
            binding()?,
            next_identity,
            RequestId::new(3)?,
            Some(next_lease),
            Some(next_identity)
        ),
        Err(ParentError::StaleRequest)
    );
    assert_eq!(
        successor.settlement_lease(
            binding()?,
            next_identity,
            RequestId::new(5001)?,
            Some(lease()?),
            Some(next_identity)
        ),
        Err(ParentError::ForeignPeer)
    );
    assert_eq!(
        successor.settlement_lease(
            binding()?,
            next_identity,
            RequestId::new(5001)?,
            Some(next_lease),
            Some(next_identity)
        )?,
        Some(next_lease)
    );
    Ok(())
}

#[test]
fn same_lease_reload_preserves_original_acquisition_and_rejects_prior_main_identity() -> Result<()>
{
    let mut parent = granted()?;
    churn(&mut parent)?;
    retire(&mut parent, identity()?)?;
    let next =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..identity()? };
    parent.main.begin(next, lease()?, moment(250))?;
    parent.acquisition.as_mut().ok_or("Origin missing")?.reload(next, lease()?, lease()?)?;
    parent.main.retire(next)?;
    parent.main.accept(
        binding()?,
        RequestId::new(4003)?,
        MainAuthorizationObservation::MainRetired { identity: next },
        moment(221),
    )?;
    parent.main.confirm_retired(next)?;
    let retained = parent.acquisition.as_ref().ok_or("Origin missing")?;
    assert!(
        retained
            .settlement_lease(
                binding()?,
                identity()?,
                RequestId::new(3)?,
                Some(lease()?),
                Some(next)
            )
            .is_err()
    );
    let budget = prepare(&mut parent, next, RequestId::new(3)?)?;
    assert_eq!(budget.settle_until, moment(235));
    assert_eq!(parent.attempt_cleanup.as_ref().ok_or("Cleanup absent")?.identity, next);
    assert_eq!(parent.acquisition.as_ref().ok_or("Origin missing")?.header, acquisition_header()?);
    assert_eq!(parent.acquisition.as_ref().ok_or("Origin missing")?.original, moment(180));
    Ok(())
}

#[test]
fn expired_cleanup_keeps_original_acquisition_and_cannot_acknowledge_or_renew() -> Result<()> {
    let mut parent = granted()?;
    churn(&mut parent)?;
    retire(&mut parent, identity()?)?;
    prepare(&mut parent, identity()?, RequestId::new(3)?)?;
    assert!(parent.expire_attempt(moment(235)).map_err(|cause| cause as Box<dyn Error>)?);
    assert!(parent.expire_attempt(moment(999)).map_err(|cause| cause as Box<dyn Error>)?);
    let retained = parent.acquisition.as_ref().ok_or("Origin lost")?;
    assert_eq!(retained.header, acquisition_header()?);
    assert_eq!(retained.original, moment(180));
    let cleanup = parent.attempt_cleanup.as_ref().ok_or("Cleanup lost")?;
    assert_eq!(cleanup.until, moment(235));
    assert_eq!(cleanup.lease, Some(lease()?));
    assert!(parent.requests.get(cleanup.index)?.reply.is_none());
    assert!(!parent.requests.get(cleanup.index)?.sent);
    assert!(parent.requests.next_reply().is_none());
    Ok(())
}

#[test]
fn no_lease_denial_survives_replay_eviction_without_inventing_backend_retirement() -> Result<()> {
    let mut parent = actor()?;
    let h = acquisition_header()?;
    let mut retained = Acquisition::new(binding()?, h, identity()?, moment(180))?;
    retained.reply(h, Reply::Rejected(LifecycleRejection::WrongPhase))?;
    complete_acquisition_write(&mut retained, h)?;
    parent.acquisition = Some(retained);
    let (index, _) = parent.requests.admit(h, b"original acquisition boundary bytes")?;
    parent.requests.get_mut(index)?.reply = Some(Reply::Rejected(LifecycleRejection::WrongPhase));
    parent.requests.get_mut(index)?.sent = true;
    churn(&mut parent)?;
    prepare(&mut parent, identity()?, RequestId::new(3)?)?;
    let cleanup = parent.attempt_cleanup.as_ref().ok_or("Cleanup missing")?;
    assert!(cleanup.lease.is_none() && cleanup.resource.is_none());
    assert!(parent.requests.get(cleanup.index)?.reply.is_none());
    assert!(!parent.native_retired());
    Ok(())
}

#[test]
fn changed_producer_lease_and_final_denial_preserve_first_original() -> Result<()> {
    let h = acquisition_header()?;
    let mut retained = Acquisition::new(binding()?, h, identity()?, moment(180))?;
    retained.observe_backend(h, RequestOutcome::Pending(lease()?))?;
    let wrong = LeaseKey { id: LeaseId::new(2)?, ..lease()? };
    assert_eq!(
        retained.observe_backend(h, RequestOutcome::Pending(wrong)),
        Err(ParentError::ForeignPeer)
    );
    retained.observe_backend(h, RequestOutcome::Failed)?;
    retained.reply(
        h,
        Reply::Failed(sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Preparation),
    )?;
    assert_eq!(
        retained.reply(h, Reply::Rejected(LifecycleRejection::WrongPhase)),
        Err(ParentError::ChangedDuplicate)
    );
    complete_acquisition_write(&mut retained, h)?;
    assert_eq!(
        retained.settlement_lease(binding()?, identity()?, RequestId::new(3)?, None, None)?,
        Some(lease()?)
    );
    assert_eq!(retained.header, h);
    assert_eq!(retained.original, moment(180));
    Ok(())
}

#[test]
fn managed_wait_owns_late_final_reply_and_one_finite_transfer_without_rebasing_header() -> Result<()>
{
    let mut parent = actor()?;
    let h = acquisition_header()?;
    let (index, _) = parent.requests.admit(h, b"actual managed original acquire")?;
    parent.requests.get_mut(index)?.until = Some(moment(180));
    parent.requests.defer_reply(index)?;
    parent.acquisition = Some(Acquisition::new(binding()?, h, identity()?, moment(180))?);
    parent.acquisition.as_mut().ok_or("Acquisition missing")?.wait_for_readiness();
    parent.expire_acquisition(moment(360)).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(!parent.acquisition.as_ref().ok_or("Acquisition missing")?.retiring());
    assert_eq!(parent.requests.next_deadline(), None);
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Acquisition missing")?.output_until(moment(390)),
        moment(390)
    );
    parent.requests.get_mut(index)?.reply = Some(Reply::Failed(
        sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Preparation,
    ));
    assert_eq!(
        parent.next_live_reply(moment(360)).map_err(|cause| cause as Box<dyn Error>)?,
        Some(index)
    );
    assert_eq!(parent.requests.next_deadline(), Some(moment(390)));
    assert_eq!(
        parent
            .requests
            .reply_deadline(index, moment(389))
            .map_err(|cause| cause as Box<dyn Error>)?,
        moment(390)
    );
    assert_eq!(parent.requests.get(index)?.header, h);
    assert_eq!(parent.requests.get(index)?.until, Some(moment(180)));
    assert!(parent.next_live_reply(moment(390)).is_err());
    Ok(())
}
#[test]
fn readiness_progress_is_frozen_and_unadmitted_owner_stays_finite() -> Result<()> {
    let h = acquisition_header()?;
    let mut managed = Acquisition::new(binding()?, h, identity()?, moment(180))?;
    managed.wait_for_readiness();
    assert_eq!(managed.progress_deadline(moment(360))?, moment(390));
    assert_eq!(managed.progress_deadline(moment(389))?, moment(390));
    assert!(!managed.expire(moment(360)));
    let mut attached = Acquisition::new(binding()?, h, identity()?, moment(180))?;
    assert_eq!(attached.output_until(moment(390)), moment(180));
    assert_eq!(attached.progress_deadline(moment(170))?, moment(180));
    assert!(attached.expire(moment(180)));
    Ok(())
}

#[test]
fn held_managed_preparation_timeout_publishes_finite_failure_without_admitting_backend()
-> Result<()> {
    use super::super::history::{HeldBackendCommand, HeldBackendTerminal};
    let mut parent = actor()?;
    let h = acquisition_header()?;
    let id = identity()?;
    let (index, _) = parent.requests.admit(h, b"configured managed acquire awaiting admission")?;
    parent.requests.get_mut(index)?.until = Some(moment(180));
    parent.requests.defer_reply(index)?;
    parent.requests.get_mut(index)?.held_backend = Some(HeldBackendCommand::Acquire(id));
    parent.acquisition = Some(Acquisition::new(binding()?, h, id, moment(180))?);
    parent.acquisition.as_mut().ok_or("Acquisition missing")?.wait_for_readiness();
    assert_eq!(parent.requests.next_deadline(), Some(moment(180)));
    assert!(
        parent
            .next_backend_command(moment(180), true, true)
            .map_err(|cause| cause as Box<dyn Error>)?
            .is_none()
    );
    assert_eq!(parent.requests.get(index)?.held_terminal, Some(HeldBackendTerminal::TimedOut));
    assert!(parent.requests.get(index)?.held_backend.is_none());
    assert_eq!(
        parent.next_live_reply(moment(180)).map_err(|cause| cause as Box<dyn Error>)?,
        Some(index)
    );
    assert!(matches!(
        parent.requests.get(index)?.reply,
        Some(Reply::Failed(sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Deadline))
    ));
    assert_eq!(
        parent
            .requests
            .reply_deadline(index, moment(200))
            .map_err(|cause| cause as Box<dyn Error>)?,
        moment(210)
    );
    assert_eq!(parent.requests.get(index)?.header, h);
    Ok(())
}

#[path = "acquisition_lifetime/reload.rs"]
mod reload;
