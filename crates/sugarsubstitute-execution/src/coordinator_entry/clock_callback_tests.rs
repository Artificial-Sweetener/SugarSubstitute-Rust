// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reenter the actual closed entry command boundary from every supplied clock issue observation.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    native_deadlines::Role,
    runtime_test_support::{Gate, Rig, TestResult},
    scope::lock,
};
use std::sync::atomic::{AtomicBool, Ordering};
use sugarsubstitute_contracts::execution::JobOutcome;
#[test]
fn supplied_clock_close_revalidates_current_connection_role_before_any_native_effect() -> TestResult
{
    for role in [Role::Handshake, Role::Read, Role::Write] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, true)?;
        let release = ReleaseBoundary(boundary.clone());
        if role == Role::Handshake {
            *lock(&boundary.pause_role) = Some(role);
        }
        let (scope, session) = server(&rig, &boundary)?;
        if role == Role::Handshake {
            fact(&rig, &session.control, |fact| {
                matches!(fact, EntryTransportObservation::Prepared)
            })?;
        } else {
            activate(&rig, &session)?;
            *lock(&boundary.pause_role) = Some(role);
            boundary.connect()?;
        }
        boundary.issue_prepared.wait()?;
        boundary.allow_retire.store(false, Ordering::Release);
        let CoordinatorEntryServerSession { control, mut completion } = session;
        let control = Arc::new(control);
        let unlocked = Arc::new(AtomicBool::new(false));
        let arrived = Arc::new(Gate::default());
        let observed = arrived.clone();
        let result = unlocked.clone();
        let closing = control.clone();
        let shared = control.shared.clone();
        let clock = rig.clock.clone();
        *lock(&boundary.clock_probe) = Some((
            role,
            Box::new(move || {
                clock.on_next_current_thread_observation(Box::new(move || {
                    let lifetime_free = shared.state.try_lock().is_ok();
                    result.store(lifetime_free, Ordering::Release);
                    if lifetime_free {
                        closing.close();
                    }
                    observed.open();
                }));
            }),
        ));
        boundary.issue_release.open();
        arrived.wait()?;
        // Old topology fails this assertion before any reentrant close can deadlock.
        if !unlocked.load(Ordering::Acquire) {
            control.close();
        }
        assert!(unlocked.load(Ordering::Acquire));
        boundary.retiring.wait()?;
        let count = match role {
            Role::Handshake => &boundary.connects_issued,
            Role::Read => &boundary.reads_issued,
            Role::Write => &boundary.writes,
        };
        assert_eq!(count.load(Ordering::Acquire), 0);
        assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
        assert!(completion.try_take_completion().is_none());
        assert_eq!(scope.snapshot().reserved_bytes, MIN_ENTRY_SERVER_BYTES);
        assert!(
            lock(&boundary.memory)
                .as_ref()
                .ok_or("Missing held actual native memory")?
                .claim(MIN_ENTRY_SERVER_BYTES)
                .is_err()
        );
        boundary.allow_retire.store(true, Ordering::Release);
        boundary.wake();
        assert!(matches!(
            rig.completion(&mut completion)?.outcome,
            JobOutcome::Success(_) | JobOutcome::Cancelled
        ));
        assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
        drop(release);
    }
    Ok(())
}

#[test]
fn same_number_context_from_foreign_execution_owner_cannot_accept_current_connection() -> TestResult
{
    let alien = Rig::new()?;
    let (_alien_scope, context) = crate::runtime_test_support::admitted_context(&alien, 2, 1)?;
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let release = ReleaseBoundary(boundary.clone());
    *lock(&boundary.pause_role) = Some(Role::Handshake);
    let (scope, mut session) = server(&rig, &boundary)?;
    assert_ne!(scope.owner_id(), context.owner_id());
    assert_eq!(scope.id(), context.scope_id());
    fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Prepared))?;
    boundary.issue_prepared.wait()?;
    *lock(&boundary.context_override) = Some(context);
    boundary.issue_release.open();
    boundary.issue_finished.wait()?;
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 0);
    session.control.close();
    assert!(matches!(
        rig.completion(&mut session.completion)?.outcome,
        JobOutcome::Success(_) | JobOutcome::Cancelled
    ));
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 0);
    drop(release);
    Ok(())
}
