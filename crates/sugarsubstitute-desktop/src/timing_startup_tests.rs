// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove fixed startup recording and invalid observations through the actual collector boundary.

use super::*;
use crate::timing::{QPC_SOURCE, Stamp, TimingClock};
use std::{cell::Cell, error::Error, rc::Rc};

type TestResult = Result<(), Box<dyn Error>>;
const STAGES: [NativeStartupStage; 7] = [
    NativeStartupStage::EventLoopReady,
    NativeStartupStage::WindowCreated,
    NativeStartupStage::PresenterReady,
    NativeStartupStage::ControlsReady,
    NativeStartupStage::ExecutionReady,
    NativeStartupStage::ShowFocusReturned,
    NativeStartupStage::FirstPreparationRequest,
];

#[derive(Clone)]
struct Counter {
    stamp: Rc<Cell<Stamp>>,
    reads: Rc<Cell<u64>>,
    failure: Rc<Cell<bool>>,
}
impl Counter {
    fn new() -> Self {
        Self {
            stamp: Rc::new(Cell::new(Stamp { ticks: 10, thread: 7, source: QPC_SOURCE })),
            reads: Rc::new(Cell::new(0)),
            failure: Rc::new(Cell::new(false)),
        }
    }
}
impl TimingClock for Counter {
    fn frequency(&self) -> i64 {
        1_000
    }
    fn thread(&self) -> u32 {
        7
    }
    fn read(&mut self) -> Result<Stamp, DesktopError> {
        self.reads.set(self.reads.get() + 1);
        if self.failure.get() {
            return Err(DesktopError::new("startup_clock", std::io::Error::other("unavailable")));
        }
        Ok(self.stamp.get())
    }
}

#[test]
fn equal_ticks_commit_seven_ordered_slots_and_later_requests_do_not_sample() -> TestResult {
    let clock = Counter::new();
    let observer = TimingObserver::new(TimingAudit::new(
        Box::new(clock.clone()),
        crate::timing::NativeStartupOrder::SurfaceFirst,
    )?);
    for stage in STAGES {
        observe_startup(Some(&observer), stage)?;
    }
    assert_eq!(clock.reads.get(), 8);
    clock.failure.set(true);
    for _ in 0..100 {
        observer.first_preparation()?;
    }
    assert_eq!(clock.reads.get(), 8);
    clock.failure.set(false);
    let receipt = observer.finish()?;
    for (slot, stage) in receipt.startup_milestones.into_iter().zip(STAGES) {
        assert_eq!(slot, Some(NativeStartupTiming { stage, ticks: 10 }));
    }
    Ok(())
}

#[test]
fn duplicate_and_out_of_order_stages_reject_before_clock_read_or_slot_mutation() -> TestResult {
    let clock = Counter::new();
    let mut audit =
        TimingAudit::new(Box::new(clock.clone()), crate::timing::NativeStartupOrder::SurfaceFirst)?;
    assert!(audit.startup(STAGES[1]).is_err());
    assert_eq!(clock.reads.get(), 1);
    assert!(audit.receipt.startup_milestones.iter().all(Option::is_none));
    audit.startup(STAGES[0])?;
    let prefix = audit.receipt.startup_milestones;
    assert!(audit.startup(STAGES[0]).is_err());
    assert!(audit.startup(STAGES[2]).is_err());
    assert_eq!(clock.reads.get(), 2);
    assert_eq!(audit.receipt.startup_milestones, prefix);
    Ok(())
}

#[test]
fn invalid_raw_startup_observations_preserve_the_last_valid_prefix() -> TestResult {
    for invalid in [
        Stamp { ticks: -1, thread: 7, source: QPC_SOURCE },
        Stamp { ticks: 9, thread: 7, source: QPC_SOURCE },
        Stamp { ticks: 10, thread: 8, source: QPC_SOURCE },
        Stamp { ticks: 10, thread: 7, source: 2 },
    ] {
        let clock = Counter::new();
        let mut audit = TimingAudit::new(
            Box::new(clock.clone()),
            crate::timing::NativeStartupOrder::SurfaceFirst,
        )?;
        audit.startup(STAGES[0])?;
        let prefix = audit.receipt.startup_milestones;
        clock.stamp.set(invalid);
        assert!(audit.startup(STAGES[1]).is_err());
        assert_eq!(audit.receipt.startup_milestones, prefix);
        assert_eq!(audit.last_tick, 10);
    }
    Ok(())
}

#[test]
fn failed_clock_preserves_cause_and_partial_teardown_receipt_keeps_only_observed_prefix()
-> TestResult {
    let clock = Counter::new();
    let mut audit =
        TimingAudit::new(Box::new(clock.clone()), crate::timing::NativeStartupOrder::SurfaceFirst)?;
    audit.startup(STAGES[0])?;
    clock.failure.set(true);
    let cause = audit.startup(STAGES[1]).err().ok_or("Clock failure missing")?;
    assert_eq!(cause.source().ok_or("Source lost")?.to_string(), "unavailable");
    clock.failure.set(false);
    let receipt = audit.finish()?;
    assert_eq!(receipt.startup_milestones[0].ok_or("Prefix lost")?.stage, STAGES[0]);
    assert!(receipt.startup_milestones[1..].iter().all(Option::is_none));
    assert!(receipt.submissions.is_empty());
    Ok(())
}

#[test]
fn disabled_startup_observation_does_not_read_or_allocate_a_collector() -> TestResult {
    assert!(
        TimingObserver::native(false, crate::timing::NativeStartupOrder::SurfaceFirst)?.is_none()
    );
    for stage in STAGES {
        observe_startup(None, stage)?;
    }
    Ok(())
}

#[test]
fn successful_submission_requires_first_actual_preparation_observation() -> TestResult {
    use sugarsubstitute_contracts::{
        execution::{Generation, SurfaceId},
        presentation::FrameIdentity,
    };
    let clock = Counter::new();
    let mut audit =
        TimingAudit::new(Box::new(clock.clone()), crate::timing::NativeStartupOrder::SurfaceFirst)?;
    let identity = FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    };
    for stage in &STAGES[..6] {
        audit.startup(*stage)?;
    }
    let begin = audit.mark()?;
    assert!(audit.submission(begin, identity).is_err());
    assert_eq!(audit.receipt.submissions_observed, 0);
    clock.stamp.set(Stamp { ticks: 20, ..clock.stamp.get() });
    audit.first_preparation()?;
    assert!(audit.submission(begin, identity).is_err());
    assert_eq!(audit.last_identity, None);
    let begin = audit.mark()?;
    audit.submission(begin, identity)?;
    assert_eq!(audit.receipt.submissions_observed, 1);
    Ok(())
}

#[test]
fn both_orders_reject_every_wrong_next_stage_without_sampling_then_record_exact_prefix()
-> TestResult {
    for order in [NativeStartupOrder::SurfaceFirst, NativeStartupOrder::ExecutionFirst] {
        let clock = Counter::new();
        let mut audit = TimingAudit::new(Box::new(clock.clone()), order)?;
        let stages = order.stages();
        for (index, next) in stages.into_iter().enumerate() {
            for wrong in stages.into_iter().filter(|candidate| *candidate != next) {
                let prefix = audit.receipt.startup_milestones;
                let reads = clock.reads.get();
                assert!(audit.startup(wrong).is_err());
                assert_eq!(clock.reads.get(), reads);
                assert_eq!(audit.receipt.startup_milestones, prefix);
            }
            audit.startup(next)?;
            assert_eq!(audit.receipt.startup_milestones[index].ok_or("Stage lost")?.stage, next);
        }
        let receipt = audit.finish()?;
        assert_eq!(receipt.startup_order, order);
        assert_eq!(
            receipt.startup_milestones.map(|slot| slot.map(|sample| sample.stage)),
            stages.map(Some)
        );
    }
    Ok(())
}
