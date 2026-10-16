// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Delay the supplied clock boundary while the production executor owns a real queued output job.
use super::super::*;
use std::{
    cell::Cell,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ScopeId, WaitFuture,
};
use sugarsubstitute_execution::{
    ExecutionOwner,
    windows_output::{OutputIdentity, WindowsOutputError},
};

thread_local! { static OBSERVING_CALLER: Cell<bool> = const { Cell::new(false) }; }
#[derive(Default)]
struct HeldClock {
    held: Mutex<bool>,
    released: Condvar,
}
impl Clock for HeldClock {
    fn now(&self) -> Moment {
        if !OBSERVING_CALLER.with(Cell::get) {
            let guard = self.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(
                self.released
                    .wait_while(guard, |held| *held)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct ClockHold {
    clock: Arc<HeldClock>,
    previous: bool,
}
impl ClockHold {
    fn new(clock: Arc<HeldClock>) -> Self {
        let previous = OBSERVING_CALLER.with(|caller| caller.replace(true));
        *clock.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        Self { clock, previous }
    }
}
impl Drop for ClockHold {
    fn drop(&mut self) {
        *self.clock.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = false;
        self.clock.released.notify_all();
        OBSERVING_CALLER.with(|caller| caller.set(self.previous));
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}

fn queued_output(
    check: impl FnOnce(&mut CoordinatorBackend, LeaseKey) -> Result<()>,
) -> Result<()> {
    let clock = Arc::new(HeldClock::default());
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let native =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&native))?;
    let hold = ClockHold::new(clock);
    let mut retained = tests::unadmitted_policy()?;
    let owner = retained.backend.as_mut();
    let run = RunKey {
        owner: tests::key()?,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let binding = ClientBinding { run, connection: ConnectionId::new(1)? };
    let peer_registration = ClientRegistration {
        run,
        connection: binding.connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("QueuedOutput_1".into())?,
    };
    owner.policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    owner.register_run(peer_registration)?;
    let root = std::env::current_dir()?;
    let target =
        BackendTarget::Managed(sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
            root.clone(),
            root.join("unopened-python.exe"),
            vec![],
            8188,
        )?);
    let request = ResourceRequest {
        run,
        connection: binding.connection,
        id: RequestId::new(1)?,
        command: ResourceCommand::Acquire(target.clone()),
    };
    let transition = owner.policy.request(binding, request.clone())?;
    let Some(RequestOutcome::Pending(lease)) =
        transition.replies.first().map(|reply| reply.outcome)
    else {
        return Err("Actual provisional lease missing".into());
    };
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
    let record = cutoff_tests::creator(lease.resource, target, budget, scope)?;
    owner.records[0] = Some(record);
    // An absent stream is the supported empty-console result, including attached
    // backends. Only an actual native batch loan may close the readiness gate.
    assert!(owner.output_loan_available(lease)?);
    assert!(owner.take_output(lease, &mut [])?.is_none());
    let record = owner.records[0].as_mut().ok_or("Original backend disappeared")?;
    record.output = Some(client.reserve_windows_output(
        &native,
        JobSpec {
            id: sugarsubstitute_contracts::execution::JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
            deadline: None,
            replacement: None,
        },
        OutputIdentity::Managed(lease.resource),
    )?);
    assert!(record.output.as_ref().ok_or("Original output disappeared")?.control.loan_available());
    assert!(owner.output_loan_available(lease)?);
    let result = check(owner, lease);
    let record = owner.records[0].as_mut().ok_or("Original backend disappeared")?;
    let output = record.output.as_mut().ok_or("Original output disappeared")?;
    assert_eq!(output.control.take_batch().err(), Some(WindowsOutputError::NotReady));
    assert!(!output.control.snapshot().prepared);
    assert_eq!(record.budget, budget);
    assert!(record.launch.is_none() && !record.started && record.readiness.is_none());
    assert_eq!(
        owner.policy.request(binding, request)?.replies.first().map(|reply| reply.outcome),
        Some(RequestOutcome::Pending(lease))
    );
    assert_eq!(native.snapshot().reserved_bytes, MIN_WINDOWS_OUTPUT_BYTES);
    output.control.close();
    drop(hold);
    loop {
        let observed = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = output.completion.try_take_completion() {
            assert!(matches!(completion.outcome, JobOutcome::Cancelled));
            break;
        }
        let guard = wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (guard, timeout) = wake
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |count| *count == observed)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(guard);
        if timeout.timed_out() {
            return Err("Original native cancellation did not complete".into());
        }
    }
    assert!(output.control.snapshot().retired);
    assert_eq!(output.control.snapshot().eof, [false, false]);
    drop(retained);
    drop(executor);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    result
}

#[test]
fn admitted_output_waiting_for_native_construction_is_idle_data() -> Result<()> {
    queued_output(|owner, key| {
        for _ in 0..3 {
            assert!(owner.take_output(key, &mut [])?.is_none());
        }
        Ok(())
    })
}
#[test]
fn foreign_output_generation_cannot_touch_the_original_queued_source() -> Result<()> {
    queued_output(|owner, key| {
        let foreign = LeaseKey {
            resource: ResourceKey { generation: key.resource.generation.next()?, ..key.resource },
            ..key
        };
        assert!(owner.output_loan_available(foreign).is_err());
        let foreign = LeaseKey {
            resource: ResourceKey {
                owner: CoordinatorKey {
                    epoch: CoordinatorEpoch::new([9; 16])?,
                    ..key.resource.owner
                },
                ..key.resource
            },
            ..key
        };
        assert!(owner.output_loan_available(foreign).is_err());
        Ok(())
    })
}
