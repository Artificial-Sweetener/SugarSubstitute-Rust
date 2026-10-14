// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real request/folder owners without manufacturing a native peer or frame loan.
#[path = "tests/acquisition_lifetime.rs"]
mod acquisition_lifetime;
#[path = "tests/acquisition_progress.rs"]
mod acquisition_progress;
#[path = "tests/attempt_cleanup.rs"]
mod attempt_cleanup;
#[path = "tests/expiry_retirement.rs"]
mod expiry_retirement;
#[path = "tests/normal_close.rs"]
mod normal_close;
#[path = "tests/ownership.rs"]
mod ownership;
use super::{ParentError, ParentLifecycle, Reply, folders::Folders, history::Requests};
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::{Generation, Moment},
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RequestId,
        RunGeneration, RunKey,
    },
    startup::{LaunchId, MainIdentity},
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FolderFragment, RawWindowsUnits, WireHeader,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Fake only native delivery facts through the actual original receipt/owner checks.
fn complete_acquisition_write(
    acquisition: &mut super::acquisition::Acquisition,
    header: WireHeader,
) -> Result<()> {
    use super::acquisition::WriteObservation;
    let mut receipt = super::write_receipt::WriteReceipt::new(7, 104);
    acquisition.observe_write(header, WriteObservation::Queued)?;
    receipt.issue(7)?;
    acquisition.observe_write(header, WriteObservation::Issued)?;
    receipt.complete(7, 104)?;
    acquisition.observe_write(header, WriteObservation::Written)?;
    Ok(())
}
#[test]
fn pending_backend_failure_keeps_exact_observed_resource_without_granted_readiness() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::{
        LeaseId, LeaseKey, RequestOutcome, ResourceGeneration, ResourceId, ResourceKey,
    };
    let id = identity()?;
    let lease = LeaseKey {
        run: id.run,
        connection: ConnectionId::new(1)?,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: id.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    };
    let mut history = Requests::new();
    let (index, _) = history.admit(header(1)?, b"pending acquire")?;
    assert_eq!(history.backend_resource(RequestId::new(1)?)?, None);
    assert!(!history.observe_backend(index, RequestOutcome::Pending(lease))?);
    let changed = LeaseKey { id: LeaseId::new(2)?, ..lease };
    assert_eq!(
        history.observe_backend(index, RequestOutcome::Pending(changed)),
        Err(ParentError::ForeignPeer)
    );
    assert!(history.observe_backend(index, RequestOutcome::Failed)?);
    assert_eq!(history.backend_resource(RequestId::new(1)?)?, Some(lease.resource));
    assert_eq!(
        history.observe_backend(index, RequestOutcome::Granted(changed)),
        Err(ParentError::ChangedDuplicate)
    );
    assert_eq!(history.backend_resource(RequestId::new(1)?)?, Some(lease.resource));
    Ok(())
}

#[test]
fn final_backend_replay_preserves_original_and_never_restarts_local_effects() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::RequestOutcome;
    let mut history = Requests::new();
    let (index, _) = history.admit(header(1)?, b"acquire")?;
    assert!(history.observe_backend(index, RequestOutcome::Failed)?);
    assert!(!history.observe_backend(index, RequestOutcome::Failed)?);
    assert_eq!(
        history.observe_backend(index, RequestOutcome::Cancelled),
        Err(ParentError::ChangedDuplicate)
    );
    assert!(!history.observe_backend(index, RequestOutcome::Failed)?);
    Ok(())
}

pub(super) fn identity() -> Result<MainIdentity> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn header(request: u64) -> Result<WireHeader> {
    Ok(WireHeader {
        run: identity()?.run,
        connection: Some(ConnectionId::new(1)?),
        request: Some(RequestId::new(request)?),
        deadline_qpc: 100,
    })
}
#[test]
fn duplicate_keeps_original_deadline_and_changed_bytes_never_admit() -> Result<()> {
    let mut history = Requests::new();
    let (index, fresh) = history.admit(header(1)?, b"original")?;
    assert!(fresh);
    history.get_mut(index)?.until = Some(Moment::from_nanos(55));
    assert_eq!(history.admit(header(1)?, b"original")?, (index, false));
    assert_eq!(history.get(index)?.until, Some(Moment::from_nanos(55)));
    assert_eq!(
        history.admit(header(1)?, b"changed").map_err(|error| error.code()),
        Err(ParentError::ChangedDuplicate)
    );
    let mut changed = header(1)?;
    changed.deadline_qpc += 1;
    assert_eq!(
        history.admit(changed, b"original").map_err(|error| error.code()),
        Err(ParentError::ChangedDuplicate)
    );
    Ok(())
}
#[test]
fn eight_live_requests_overload_before_advancing_high_water() -> Result<()> {
    let mut history = Requests::new();
    for id in 1..=8 {
        assert!(history.admit(header(id)?, &[id as u8])?.1);
    }
    assert_eq!(
        history.admit(header(9)?, &[9]).map_err(|error| error.code()),
        Err(ParentError::Overloaded)
    );
    history.get_mut(0)?.reply = Some(Reply::Acknowledged);
    history.get_mut(0)?.sent = true;
    assert!(history.admit(header(9)?, &[9])?.1);
    assert_eq!(
        history.admit(header(1)?, &[1]).map_err(|error| error.code()),
        Err(ParentError::StaleRequest)
    );
    Ok(())
}
#[test]
fn out_of_order_new_request_and_foreign_connection_duplicates_preserve_original() -> Result<()> {
    let mut history = Requests::new();
    let (index, _) = history.admit(header(7)?, b"seven")?;
    assert_eq!(
        history.admit(header(6)?, b"six").map_err(|error| error.code()),
        Err(ParentError::StaleRequest)
    );
    let mut foreign = header(7)?;
    foreign.connection = Some(ConnectionId::new(2)?);
    assert_eq!(
        history.admit(foreign, b"seven").map_err(|error| error.code()),
        Err(ParentError::ChangedDuplicate)
    );
    assert_eq!(history.get(index)?.header, header(7)?);
    Ok(())
}
#[test]
fn raw_requested_surrogates_match_exactly_before_effective_units_are_retained() -> Result<()> {
    let id = identity()?;
    let requested = [0x0045, 0x003a, 0x005c, 0xd800];
    let fragment = FolderFragment::new(id, 4, 0, RawWindowsUnits::from_units(&requested)?)?;
    let mut folders = Folders::new();
    folders.begin(id);
    assert_eq!(folders.effective(fragment, requested.len()), Err(ParentError::ProfileMismatch));
    let changed = [0x0045, 0x003a, 0x005c, 0xfffd];
    assert_eq!(folders.requested(fragment, &changed), Err(ParentError::ProfileMismatch));
    assert_eq!(folders.requested(fragment, &requested)?, 4);
    assert!(folders.effective(fragment, requested.len())?);
    assert_eq!(folders.units(), &requested);
    Ok(())
}
#[test]
fn original_two_fragments_cannot_be_reordered_or_rebound() -> Result<()> {
    let id = identity()?;
    let requested = vec![0x0041; 16385];
    let first =
        FolderFragment::new(id, 16385, 0, RawWindowsUnits::from_units(&requested[..16384])?)?;
    let last =
        FolderFragment::new(id, 16385, 16384, RawWindowsUnits::from_units(&requested[16384..])?)?;
    let mut folders = Folders::new();
    folders.begin(id);
    assert_eq!(folders.requested(last, &requested), Err(ParentError::ProfileMismatch));
    assert_eq!(folders.requested(first, &requested)?, 16384);
    assert_eq!(folders.requested(last, &requested)?, 16385);
    assert_eq!(folders.effective(last, requested.len()), Err(ParentError::ProfileMismatch));
    assert!(!folders.effective(first, requested.len())?);
    assert!(folders.effective(last, requested.len())?);
    assert_eq!(folders.units(), requested.as_slice());
    Ok(())
}
#[test]
fn successor_resets_folder_progress_but_cannot_reuse_old_identity() -> Result<()> {
    let old = identity()?;
    let next = MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..old };
    let units = [0x0041];
    let mut folders = Folders::new();
    folders.begin(old);
    folders
        .requested(FolderFragment::new(old, 1, 0, RawWindowsUnits::from_units(&units)?)?, &units)?;
    folders.begin(next);
    assert_eq!(
        folders.requested(
            FolderFragment::new(old, 1, 0, RawWindowsUnits::from_units(&units)?)?,
            &units
        ),
        Err(ParentError::ProfileMismatch)
    );
    assert_eq!(
        folders.requested(
            FolderFragment::new(next, 1, 0, RawWindowsUnits::from_units(&units)?)?,
            &units
        )?,
        1
    );
    Ok(())
}
#[test]
fn actor_inline_peak_leaves_bounded_copy_and_control_capacity() {
    // Includes eight32KiB replay buffers and the complete32766-unit profile folder.
    let retained = size_of::<ParentLifecycle>();
    assert!(retained < 384 * 1024);
    assert!(3 * retained + 512 * 1024 < ParentLifecycle::storage_bytes() as usize);
}

#[test]
fn completion_before_native_issue_or_wrong_count_keeps_original_receipt() -> Result<()> {
    let mut receipt = super::write_receipt::WriteReceipt::new(7, 256);
    assert_eq!(receipt.complete(7, 256), Err(ParentError::WrongPhase));
    assert_eq!(receipt.issue(8), Err(ParentError::WrongPhase));
    receipt.issue(7)?;
    assert_eq!(receipt.complete(7, 255), Err(ParentError::WrongPhase));
    assert_eq!(receipt.complete(8, 256), Err(ParentError::WrongPhase));
    receipt.complete(7, 256)?;
    Ok(())
}
#[test]
fn duplicate_issue_and_revocation_after_issue_cannot_release_actual_pending_write() -> Result<()> {
    let mut receipt = super::write_receipt::WriteReceipt::new(4, 104);
    receipt.issue(4)?;
    assert_eq!(receipt.issue(4), Err(ParentError::WrongPhase));
    assert_eq!(receipt.revoked(4), Err(ParentError::WrongPhase));
    receipt.complete(4, 104)?;
    Ok(())
}
