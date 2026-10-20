// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real registry/executor force requires the original explicit stop transaction.
use super::*;
#[test]
fn original_grace_cannot_force_before_the_owned_stop_is_admitted() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    h.created(ticket.process())?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    h.clock.0.store(budget.graceful_until.as_nanos(), Ordering::Release);
    assert_eq!(h.registry.snapshot(ticket.process())?.shutdown, None);
    assert!(!h.registry.snapshot(ticket.process())?.cleanup_step_due(h.client.now()?));
    let mut rejected = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    rejected.deadline = Some(budget.settle_until);
    assert!(matches!(
        h.registry.submit_cleanup_step(&h.client, ticket.process(), &h.scope, rejected, None),
        Err(RegistryAdmissionError::Closed)
    ));
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    h.registry.begin_stop(ticket.process(), ShutdownId::new(1)?, budget)?;
    let watch = if h.registry.snapshot(ticket.process())?.member_watch_required {
        Some(h.spec(JobClass::NativeObservation, 4096)?)
    } else {
        None
    };
    let mut cleanup = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    cleanup.deadline = Some(budget.settle_until);
    h.registry.submit_cleanup_step(&h.client, ticket.process(), &h.scope, cleanup, watch)?;
    h.drain_until(|registry| {
        registry.snapshot(ticket.process()).is_ok_and(|snapshot| !snapshot.cleanup_step_pending)
    })?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    assert_eq!(
        h.registry.snapshot(ticket.process())?.shutdown,
        Some((ShutdownId::new(1)?, budget))
    );
    assert!(h.registry.snapshot(ticket.process())?.forced);
    Ok(())
}

/// A dependent helper keeps its fixed grace after the backend's earlier force boundary.
#[test]
fn dependent_cleanup_phase_preserves_total_and_does_not_force_early() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    h.created(ticket.process())?;
    let original = ShutdownBudget::new(Moment::from_nanos(0))?;
    let budget = ShutdownBudget { graceful_until: Moment::from_nanos(12_000_000_000), ..original };
    let mut extended = budget;
    extended.settle_until = Moment::from_nanos(16_000_000_000);
    assert!(h.registry.begin_stop(ticket.process(), ShutdownId::new(1)?, extended).is_err());
    let mut reversed = budget;
    reversed.graceful_until = Moment::from_nanos(16_000_000_000);
    assert!(h.registry.begin_stop(ticket.process(), ShutdownId::new(1)?, reversed).is_err());
    h.registry.begin_stop(ticket.process(), ShutdownId::new(1)?, budget)?;
    assert!(h.registry.begin_stop(ticket.process(), ShutdownId::new(1)?, original).is_err());
    for (now, forces) in [(original.graceful_until, 0), (budget.graceful_until, 1)] {
        h.clock.0.store(now.as_nanos(), Ordering::Release);
        let watch = if h.registry.snapshot(ticket.process())?.member_watch_required {
            Some(h.spec(JobClass::NativeObservation, 4096)?)
        } else {
            None
        };
        let mut cleanup = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
        cleanup.deadline = Some(budget.settle_until);
        h.registry.submit_cleanup_step(&h.client, ticket.process(), &h.scope, cleanup, watch)?;
        h.drain_until(|registry| {
            registry.snapshot(ticket.process()).is_ok_and(|snapshot| !snapshot.cleanup_step_pending)
        })?;
        assert_eq!(h.fake.forces.load(Ordering::Acquire), forces);
        assert_eq!(
            h.registry.snapshot(ticket.process())?.shutdown,
            Some((ShutdownId::new(1)?, budget))
        );
    }
    Ok(())
}
