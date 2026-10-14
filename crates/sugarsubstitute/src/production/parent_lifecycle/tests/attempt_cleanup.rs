// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expired cleanup stays semantic while actual transport/native custody remains unresolved.
use super::super::attempt::{AttemptCleanupFailure, AttemptSettlement};
use super::*;
#[test]
fn expired_original_attempt_quarantines_reply_without_closing_or_acknowledging_transport()
-> Result<()> {
    let id = identity()?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    let mut actor = ParentLifecycle::new(
        id.run,
        id.launch,
        sugarsubstitute_contracts::configuration::ProfileId::new("Attempt_1".into())?,
        ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        storage,
    )?;
    let h = header(2)?;
    let (index, _) = actor.requests.admit(h, b"same exact settlement")?;
    let until = Moment::from_nanos(15_000_000_000);
    actor.requests.get_mut(index)?.until = Some(until);
    actor.attempt_cleanup = Some(AttemptSettlement {
        index,
        resource: None,
        lease: None,
        origin: header(1)?,
        identity: id,
        until,
        failed: None,
        main_retirement_required: false,
        retirement: None,
        ready: false,
    });
    assert!(
        !actor
            .expire_attempt(Moment::from_nanos(until.as_nanos() - 1))
            .map_err(|cause| cause as Box<dyn Error>)?
    );
    assert_eq!(actor.requests.next_deadline(), Some(until));
    assert!(actor.expire_attempt(until).map_err(|cause| cause as Box<dyn Error>)?);
    assert!(!actor.closed);
    assert!(!actor.native_retired());
    assert!(actor.failure().is_none());
    assert!(actor.settled_attempt_failure().is_none());
    assert!(actor.requests.get(index)?.reply.is_none());
    assert!(!actor.requests.get(index)?.sent);
    assert_eq!(actor.requests.get(index)?.until, Some(until));
    assert_eq!(actor.requests.next_deadline(), None);
    assert_eq!(actor.requests.next_reply(), None);
    assert_eq!(actor.requests.admit(h, b"same exact settlement")?, (index, false));
    assert_eq!(
        actor.attempt_cleanup.as_ref().and_then(|a| a.failed),
        Some(AttemptCleanupFailure::Deadline)
    );
    assert!(
        actor
            .expire_attempt(Moment::from_nanos(999_000_000_000))
            .map_err(|cause| cause as Box<dyn Error>)?
    );
    assert_eq!(actor.attempt_cleanup.as_ref().map(|a| a.until), Some(until));
    // A fresh cancellation request can coexist with the retained failed correlation.
    assert!(actor.requests.admit(header(3)?, b"user cancel")?.1);
    Ok(())
}
#[test]
fn abandoned_request_keeps_digest_highwater_and_cannot_be_replied_or_evicted_as_sent() -> Result<()>
{
    let mut history = Requests::new();
    for id in 1..=8 {
        let (index, _) = history.admit(header(id)?, &[id as u8])?;
        history.get_mut(index)?.until = Some(Moment::from_nanos(100));
        history.abandon(index)?;
    }
    assert_eq!(history.next_deadline(), None);
    assert_eq!(
        history.admit(header(9)?, b"new").map_err(|e| e.code()),
        Err(ParentError::Overloaded)
    );
    assert_eq!(
        history.admit(header(1)?, b"changed").map_err(|e| e.code()),
        Err(ParentError::ChangedDuplicate)
    );
    assert!(!history.admit(header(8)?, &[8])?.1);
    history.get_mut(7)?.reply = Some(Reply::Acknowledged);
    assert_eq!(history.next_reply(), None);
    Ok(())
}

#[test]
fn original_grant_requires_retirement_even_before_first_requested_folder() -> Result<()> {
    use sugarsubstitute_application::supervision::{
        MainAuthorizationObservation as Observation, MainAuthorizationSession,
        MainAuthorizationStage,
    };
    use sugarsubstitute_contracts::lifecycle::{
        ClientBinding, LeaseId, LeaseKey, ResourceGeneration, ResourceId, ResourceKey,
    };
    let id = identity()?;
    let binding = ClientBinding { run: id.run, connection: ConnectionId::new(1)? };
    let lease = LeaseKey {
        run: id.run,
        connection: binding.connection,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: id.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    };
    let mut main = MainAuthorizationSession::new(id.run);
    assert_eq!(main.stage(), MainAuthorizationStage::Retired);
    let original = Moment::from_nanos(30_000_000_000);
    main.begin(id, lease, original)?;
    assert_eq!(main.stage(), MainAuthorizationStage::RequestedFolder);
    assert_eq!(main.snapshot().frontend_until, Some(original));
    main.retire(id)?;
    assert!(
        main.accept(
            binding,
            RequestId::new(2)?,
            Observation::RequestedFolderAccepted { identity: id, next_offset_units: 1 },
            Moment::from_nanos(1)
        )
        .is_err()
    );
    assert_eq!(main.stage(), MainAuthorizationStage::Retiring);
    assert!(main.confirm_retired(id).is_err());
    main.accept(
        binding,
        RequestId::new(3)?,
        Observation::MainRetired { identity: id },
        Moment::from_nanos(31_000_000_000),
    )?;
    main.confirm_retired(id)?;
    assert_eq!(main.stage(), MainAuthorizationStage::Retired);
    assert_eq!(main.snapshot().frontend_until, Some(original));
    Ok(())
}
#[test]
fn first_requested_folder_after_grant_uses_original_parent_attempt_and_retains_wire_history()
-> Result<()> {
    use sugarsubstitute_application::supervision::{
        MainAuthorizationObservation as Observation, MainAuthorizationSession,
        MainAuthorizationStage,
    };
    use sugarsubstitute_contracts::lifecycle::{
        ClientBinding, LeaseId, LeaseKey, RequestOutcome, ResourceGeneration, ResourceId,
        ResourceKey,
    };
    let id = identity()?;
    let binding = ClientBinding { run: id.run, connection: ConnectionId::new(1)? };
    let lease = LeaseKey {
        run: id.run,
        connection: binding.connection,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: id.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    };
    let mut history = Requests::new();
    let (acquisition, _) = history.admit(header(1)?, b"original acquisition")?;
    history.get_mut(acquisition)?.identity = Some(id);
    history.get_mut(acquisition)?.until = Some(Moment::from_nanos(180_000_000_000));
    assert!(!history.observe_backend(acquisition, RequestOutcome::Pending(lease))?);
    assert!(history.observe_backend(acquisition, RequestOutcome::Granted(lease))?);
    history.get_mut(acquisition)?.reply = Some(Reply::Granted { lease, identity: id });
    history.get_mut(acquisition)?.sent = true;
    let mut main = MainAuthorizationSession::new(id.run);
    let original = Moment::from_nanos(206_000_000_000);
    main.begin(id, lease, original)?;
    let mut folders = Folders::new();
    folders.begin(id);
    let units = [0x0045, 0x003a, 0x005c, 0xd800];
    let fragment = FolderFragment::new(id, 4, 0, RawWindowsUnits::from_units(&units)?)?;
    let next = folders.requested(fragment, &units)?;
    assert!(
        main.accept(
            binding,
            RequestId::new(2)?,
            Observation::RequestedFolderAccepted { identity: id, next_offset_units: next },
            Moment::from_nanos(177_000_000_000)
        )?
        .is_none()
    );
    assert_eq!(main.stage(), MainAuthorizationStage::RequestedFolder);
    assert_eq!(main.snapshot().frontend_until, Some(original));
    assert_eq!(history.backend_lease(RequestId::new(1)?)?, Some(lease));
    assert_eq!(history.get(acquisition)?.until, Some(Moment::from_nanos(180_000_000_000)));
    assert!(folders.units().is_empty());
    Ok(())
}
