// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive reservations from the real coordinator; fake only external write facts.
use super::super::acquisition::Acquisition;
use super::*;
use sugarsubstitute_application::supervision::{MainAuthorizationStage, ResourceCoordinator};
use sugarsubstitute_contracts::{
    configuration::{LaunchConfiguration, ProfileId},
    lifecycle::*,
};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

pub(super) fn pending() -> Result<(ParentLifecycle, WireHeader, LeaseKey, ResourceCoordinator)> {
    pending_for(identity()?)
}
pub(super) fn pending_for(
    id: MainIdentity,
) -> Result<(ParentLifecycle, WireHeader, LeaseKey, ResourceCoordinator)> {
    let binding = ClientBinding { run: id.run, connection: ConnectionId::new(1)? };
    let mut coordinator = ResourceCoordinator::new(id.run.owner);
    let peer_registration = ClientRegistration {
        run: id.run,
        connection: binding.connection,
        launch: id.launch,
        profile: ProfileId::new("Progress_1".into())?,
    };
    coordinator.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    coordinator.register_run(peer_registration)?;
    let root = std::env::current_dir()?;
    let transition = coordinator.request(
        binding,
        ResourceRequest {
            run: id.run,
            connection: binding.connection,
            id: RequestId::new(3)?,
            command: ResourceCommand::Acquire(BackendTarget::Managed(LaunchConfiguration::new(
                root.clone(),
                root.join("inert-python.exe"),
                vec![],
                8188,
            )?)),
        },
    )?;
    let reply = *transition.replies.first().ok_or("Actual Pending absent")?;
    let RequestOutcome::Pending(lease) = reply.outcome else {
        return Err("Reservation not Pending".into());
    };
    let h = WireHeader { run: id.run, deadline_qpc: 180_000_000_000, ..header(3)? };
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    let mut parent = ParentLifecycle::new(
        id.run,
        id.launch,
        ProfileId::new("Progress_1".into())?,
        binding.connection,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?),
    )?;
    let (index, _) = parent.requests.admit(h, b"original pending acquisition")?;
    parent.requests.acquisition(index, h.request.ok_or("Origin absent")?)?;
    parent.requests.get_mut(index)?.until = Some(Moment::from_nanos(180_000_000_000));
    parent.requests.observe_backend(index, reply.outcome)?;
    let mut acquisition = Acquisition::new(
        binding,
        h,
        id,
        Moment::from_nanos(180_000_000_000),
        sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
    )?;
    acquisition.observe_backend(h, reply.outcome)?;
    parent.acquisition = Some(acquisition);
    Ok((parent, h, lease, coordinator))
}
pub(super) fn copied_binding(lease: LeaseKey) -> ClientBinding {
    ClientBinding { run: lease.run, connection: lease.connection }
}
/// External issue/completion facts traverse the actual scalar receipt validator.
/// This creates no native write admission, peer, frame, readiness or release witness.
pub(super) fn complete_progress(parent: &mut ParentLifecycle, h: WireHeader) -> Result<()> {
    parent.acquisition.as_mut().ok_or("Acquisition absent")?.progress_queued(h)?;
    let mut receipt = super::super::write_receipt::WriteReceipt::new(7, 129);
    assert!(receipt.complete(7, 129).is_err());
    receipt.issue(7)?;
    assert!(receipt.complete(7, 128).is_err());
    receipt.complete(7, 129)?;
    parent.acquisition.as_mut().ok_or("Acquisition absent")?.progress_finished(h, true)?;
    Ok(())
}

#[test]
fn checked_nonfinal_progress_keeps_acquire_live_through_output_replay_churn() -> Result<()> {
    let (mut parent, h, lease, _) = pending()?;
    let reference = super::super::replies::reference(lease);
    assert_eq!(parent.acquisition.as_ref().ok_or("Missing")?.progress(), Some(lease));
    assert!(
        parent
            .acquisition
            .as_ref()
            .ok_or("Missing")?
            .output_lease(copied_binding(lease), reference)
            .is_err()
    );
    parent.begin_output_sequence(lease)?;
    complete_progress(&mut parent, h)?;
    assert_eq!(
        parent
            .acquisition
            .as_ref()
            .ok_or("Missing")?
            .output_lease(copied_binding(lease), reference)?,
        lease
    );
    assert!(parent.lease(reference).is_err());
    assert_eq!(parent.main.stage(), MainAuthorizationStage::Retired);
    assert_eq!(parent.main.snapshot().identity, None);
    for number in 4..=4000 {
        let request = WireHeader { request: Some(RequestId::new(number)?), ..h };
        let mut bytes = [0; 512];
        let count = FrameCodec::encode(
            &WireMessage {
                header: request,
                payload: WirePayload::OutputPull { lease: reference, acknowledged_delivery: None },
            },
            &mut bytes,
        )?;
        let (index, fresh) = parent.requests.admit(request, &bytes[..count])?;
        assert!(fresh);
        parent.requests.get_mut(index)?.reply = Some(Reply::OutputIdle(lease));
        parent.requests.get_mut(index)?.sent = true;
    }
    let index =
        parent.requests.find(h.request.ok_or("Request absent")?).ok_or("Original slot evicted")?;
    assert!(!parent.requests.get(index)?.sent);
    assert!(parent.requests.get(index)?.reply.is_none());
    assert_eq!(parent.acquisition.as_ref().ok_or("Missing")?.progress(), None);
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.original,
        Moment::from_nanos(180_000_000_000)
    );
    Ok(())
}

#[test]
fn provisional_source_rejects_foreign_changed_unwritten_and_successor_correlation() -> Result<()> {
    let (mut parent, h, lease, _) = pending()?;
    let reference = super::super::replies::reference(lease);
    let acq = parent.acquisition.as_mut().ok_or("Missing")?;
    assert!(acq.progress_finished(h, true).is_err());
    assert!(acq.progress_queued(WireHeader { deadline_qpc: h.deadline_qpc + 1, ..h }).is_err());
    acq.progress_queued(h)?;
    assert!(acq.progress_queued(h).is_err());
    acq.progress_finished(h, false)?;
    assert_eq!(acq.progress(), Some(lease));
    complete_progress(&mut parent, h)?;
    let acq = parent.acquisition.as_mut().ok_or("Missing")?;
    assert!(acq.progress_finished(h, true).is_err());
    let mut foreign = copied_binding(lease);
    foreign.connection = ConnectionId::new(2)?;
    assert!(acq.output_lease(foreign, reference).is_err());
    assert!(
        acq.output_lease(
            copied_binding(lease),
            sugarsubstitute_supervision::lifecycle_protocol::LeaseReference {
                generation: ResourceGeneration::new(2)?,
                ..reference
            }
        )
        .is_err()
    );
    let next = Acquisition::new(
        copied_binding(lease),
        WireHeader { request: Some(RequestId::new(4001)?), ..h },
        identity()?,
        acq.original,
        sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
    )?;
    assert!(next.output_lease(copied_binding(lease), reference).is_err());
    Ok(())
}

#[test]
fn same_source_grant_adoption_preserves_delivery_sequence_and_consumed_ack() -> Result<()> {
    let (mut parent, h, lease, _) = pending()?;
    parent.begin_output_sequence(lease)?;
    complete_progress(&mut parent, h)?;
    let ack = std::num::NonZeroU64::new(17).ok_or("ACK absent")?;
    parent.next_delivery = 17;
    parent.acknowledged_output = Some((lease, ack));
    // This tests data-owner adoption only. No backend readiness observation is fabricated.
    parent.begin_output_sequence(lease)?;
    assert_eq!(parent.next_delivery, 17);
    assert_eq!(parent.acknowledged_output, Some((lease, ack)));
    let mut foreign = lease;
    foreign.id = LeaseId::new(lease.id.get() + 1)?;
    assert!(parent.begin_output_sequence(foreign).is_err());
    assert_eq!(parent.output_lease, Some(lease));
    assert!(parent.lease(super::super::replies::reference(lease)).is_err());
    Ok(())
}

#[test]
fn original_pending_settlement_requires_written_progress_and_exact_attempt() -> Result<()> {
    let (mut parent, h, lease, _) = pending()?;
    let request = h.request.ok_or("Request absent")?;
    let acq = parent.acquisition.as_ref().ok_or("Missing")?;
    assert!(acq.settlement_lease(copied_binding(lease), identity()?, request, None, None).is_err());
    complete_progress(&mut parent, h)?;
    let acq = parent.acquisition.as_ref().ok_or("Missing")?;
    assert_eq!(
        acq.settlement_lease(copied_binding(lease), identity()?, request, None, None)?,
        Some(lease)
    );
    assert!(
        acq.settlement_lease(copied_binding(lease), identity()?, RequestId::new(2)?, None, None)
            .is_err()
    );
    assert!(
        acq.settlement_lease(
            copied_binding(lease),
            MainIdentity { navigation: Generation::new(2)?, ..identity()? },
            request,
            None,
            None
        )
        .is_err()
    );
    assert!(
        acq.settlement_lease(copied_binding(lease), identity()?, request, Some(lease), None)
            .is_err()
    );
    // Expired cleanup cannot acknowledge or erase the retained reservation.
    let cleanup_header = WireHeader { request: Some(RequestId::new(4001)?), ..h };
    let (index, _) = parent.requests.admit(cleanup_header, b"original expired cleanup")?;
    assert!(
        parent
            .prepare_attempt_cleanup(
                index,
                identity()?,
                request,
                Moment::from_nanos(200),
                Moment::from_nanos(200),
                false
            )
            .is_err()
    );
    assert!(parent.attempt_cleanup.is_none());
    assert!(parent.requests.get(index)?.reply.is_none());
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.original,
        Moment::from_nanos(180_000_000_000)
    );
    Ok(())
}

#[path = "acquisition_progress/failure.rs"]
mod failure;

#[path = "acquisition_progress/receipt.rs"]
pub(super) mod receipt;
