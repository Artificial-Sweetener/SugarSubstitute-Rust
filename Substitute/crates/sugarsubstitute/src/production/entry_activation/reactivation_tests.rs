// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A repeated launch learns whether the existing window answers from a probe or the watchdog.
use super::*;
use crate::completion_wake::Wake;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    Clock, ExecutionPort, Moment, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

struct ClockPort;
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

fn hung(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    Ok(ProcessResponsiveness::Unresponsive)
}

#[test]
fn unpresented_owner_cannot_acknowledge_a_repeated_launch() -> Result<()> {
    let restored = std::cell::Cell::new(false);
    let replied = std::cell::Cell::new(false);
    assert!(matches!(
        restore_and_reply(
            None,
            |_| {
                restored.set(true);
                Ok(true)
            },
            || {
                replied.set(true);
                Ok(())
            }
        )?,
        Reuse::Waiting
    ));
    assert!(!restored.get() && !replied.get());
    Ok(())
}

/// Only a completed probe or the watchdog's latched suspicion decides; no answer means probe.
#[test]
fn responsiveness_decides_between_bringing_forward_and_reporting_unresponsive() {
    use ProcessResponsiveness::*;
    assert_eq!(decide(false, true, None), Decision::Probe);
    assert_eq!(decide(false, true, Some(Responsive)), Decision::Restore);
    assert_eq!(decide(false, true, Some(Unresponsive)), Decision::Unresponsive);
    assert_eq!(decide(false, true, Some(Exited)), Decision::Wait);
    assert_eq!(decide(false, false, None), Decision::Wait);
    for probed in [None, Some(Responsive), Some(Unresponsive), Some(Exited)] {
        assert_eq!(decide(true, true, probed), Decision::Unresponsive, "suspected {probed:?}");
        assert_eq!(decide(true, false, probed), Decision::Unresponsive, "suspected {probed:?}");
    }
}

/// A hung probe answers Unresponsive through an owned blocking job, and the answer is kept so
/// later turns decide without probing again.
#[test]
fn a_hung_probe_answers_unresponsive_through_its_owned_job() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(ClockPort), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut probe = ResponsivenessProbe::default();
    assert_eq!(probe.observe((7, 11), &client, &scope, &jobs, hung)?, None);
    let answer = wake.until(|| {
        client.drain_completions(std::num::NonZeroUsize::MIN).ok()?;
        probe.observe((7, 11), &client, &scope, &jobs, hung).ok().flatten()
    });
    assert_eq!(answer, ProcessResponsiveness::Unresponsive);
    assert_eq!(decide(false, true, Some(answer)), Decision::Unresponsive);
    assert!(probe.job.is_none());
    assert_eq!(
        probe.observe((7, 11), &client, &scope, &jobs, |_, _| Ok(
            ProcessResponsiveness::Responsive
        ))?,
        Some(ProcessResponsiveness::Unresponsive)
    );
    probe.reset();
    assert_eq!(probe.answer, None);
    drop(scope);
    drop(executor);
    Ok(())
}
