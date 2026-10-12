// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer only real authenticated frame custody across shape failures and retirement.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::JobOutcome;

#[test]
fn startup_snapshot_layout_preserves_complete_original_custody_on_both_branches() {
    let launch = std::mem::size_of::<AuthenticatedEntryLaunch>();
    let prepared = std::mem::size_of::<PreparedStartupSnapshot>();
    let rejection = std::mem::size_of::<StartupSnapshotRejection>();
    let original = std::mem::size_of::<Result<PreparedStartupSnapshot, StartupSnapshotRejection>>();
    let boxed =
        std::mem::size_of::<Result<PreparedStartupSnapshot, Box<StartupSnapshotRejection>>>();
    eprintln!(
        "startup_snapshot_layout launch={launch} prepared={prepared} rejection={rejection} original_result={original} boxed_error_result={boxed}"
    );
    assert!(prepared >= launch);
    assert!(rejection >= launch);
    assert!(boxed >= prepared);
    assert!(original >= prepared);
    assert!(original >= rejection);
}

fn launch(
    rig: &Rig,
    boundary: &std::sync::Arc<Boundary>,
    session: &CoordinatorEntryServerSession,
) -> TestResult<AuthenticatedEntryLaunch> {
    activate(rig, session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { request, .. } =
        fact(rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("missing actual authenticated launch".into());
    };
    Ok(request)
}

#[test]
fn startup_snapshot_moves_original_allocation_peer_counter_and_connection_loan() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let request = launch(&rig, &boundary, &session)?;
    let pointer = request.frame.bytes().as_ptr();
    let index = request.frame.index;
    let shared = request.frame.shared.clone();
    let id = request.request_id();
    let received = request.received_at();
    let until = request.until();
    let source = request.source_deadline();
    let peer = format!("{:?}", request.peer());
    let snapshot = request.into_startup_snapshot().map_err(|failure| failure.error)?;
    assert_eq!(snapshot.launch.frame.bytes().as_ptr(), pointer);
    assert_eq!(snapshot.body, protocol::HEADER_BYTES..snapshot.launch.frame.length);
    assert_eq!(snapshot.launch.frame.index, index);
    assert!(std::sync::Arc::ptr_eq(&snapshot.launch.frame.shared, &shared));
    assert_eq!(snapshot.launch.request_id(), id);
    assert_eq!(snapshot.launch.received_at(), received);
    assert_eq!(snapshot.launch.until(), until);
    assert_eq!(snapshot.launch.source_deadline(), source);
    assert_eq!(format!("{:?}", snapshot.launch.peer()), peer);
    assert!(lock(&shared.state).connections[index].loan);
    assert_eq!(
        protocol::launch_payload::decode(&snapshot.launch.frame.bytes()[snapshot.body.clone()])?
            .profile(),
        "profile_1"
    );
    drop(snapshot);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}

#[test]
fn startup_snapshot_retains_actual_frame_charge_until_native_close_and_loan_return() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let snapshot = launch(&rig, &boundary, &session)?
        .into_startup_snapshot()
        .map_err(|failure| failure.error)?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing original memory lease")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    assert_eq!(snapshot.launch.view()?.profile(), "profile_1");
    drop(snapshot);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

fn rejected_shape(change: impl FnOnce(&mut AuthenticatedEntryLaunch)) -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let mut request = launch(&rig, &boundary, &session)?;
    let pointer = request.frame.bytes().as_ptr();
    let index = request.frame.index;
    let shared = request.frame.shared.clone();
    let source = request.source_deadline();
    change(&mut request);
    let rejection = request.into_startup_snapshot().err().ok_or("invalid snapshot admitted")?;
    assert!(matches!(rejection.error, EntryTransportError::Malformed));
    assert_eq!(
        rejection.launch.frame.buffer.as_ref().ok_or("lost original frame")?.bytes.as_ptr(),
        pointer
    );
    assert_eq!(rejection.launch.frame.index, index);
    assert_eq!(rejection.launch.source_deadline(), source);
    assert!(std::sync::Arc::ptr_eq(&rejection.launch.frame.shared, &shared));
    assert!(lock(&shared.state).connections[index].loan);
    drop(rejection);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}

#[test]
fn startup_snapshot_rejects_truncated_frame_and_returns_exact_original_loan() -> TestResult {
    rejected_shape(|launch| launch.frame.length = protocol::HEADER_BYTES)
}

#[test]
fn startup_snapshot_rejects_changed_header_and_returns_exact_original_loan() -> TestResult {
    rejected_shape(|launch| {
        if let Some(buffer) = launch.frame.buffer.as_mut() {
            buffer.bytes[0] ^= 1;
        }
    })
}

#[test]
fn startup_snapshot_rejects_changed_body_and_returns_exact_original_loan() -> TestResult {
    rejected_shape(|launch| {
        if let Some(buffer) = launch.frame.buffer.as_mut() {
            buffer.bytes[protocol::HEADER_BYTES + 8] = 1;
        }
    })
}
