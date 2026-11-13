// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual admission rejection, original clocks and late cancellation.
use super::*;
struct ForeignRelease;
impl MemoryRelease for ForeignRelease {
    fn release(&self, _: u64) {}
}
#[derive(Default)]
struct CounterPorts(std::sync::atomic::AtomicUsize);
impl CancelControl for CounterPorts {
    fn is_requested(&self) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        true
    }
    fn cancelled(&self) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
impl Clock for CounterPorts {
    fn now(&self) -> Moment {
        self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[test]
fn registered_startup_counterfeit_matching_context_and_bare_memory_do_not_consume_original_issuance()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let mut startup = request_snapshot(&rig, &boundary, &session)?;
    let profile = request()?.profile_directory().to_path_buf();
    let until = startup.launch.as_ref().ok_or("Original Entry missing")?.until();
    let client = rig.client.clone();
    let original = run()?;
    let mut job =
        rig.client.submit_blocking(&rig.scope, projection_spec(2, until)?, move |context| {
            let ports = Arc::new(CounterPorts::default());
            let counterfeit = JobContext::new(
                context.owner_id(),
                context.scope_id(),
                context.spec(),
                context.deadline(),
                ports.clone(),
                ports.clone(),
                AdmittedJobResources::new(MemoryLease::new(BYTES, Box::new(ForeignRelease)), None),
            );
            assert!(matches!(
                client.register_startup_source(&mut startup, &counterfeit, original, &profile),
                Err(AdmissionError::InvalidCaptureAdmission)
            ));
            assert_eq!(ports.0.load(std::sync::atomic::Ordering::Acquire), 0);
            let exhausted = context
                .memory()
                .claim(BYTES - REGISTERED_STARTUP_BYTES + 1)
                .map_err(AdmissionError::Memory)?;
            assert!(matches!(
                client.register_startup_source(&mut startup, &context, original, &profile),
                Err(AdmissionError::Memory(_))
            ));
            drop(exhausted);
            let source =
                client.register_startup_source(&mut startup, &context, original, &profile)?;
            Ok::<_, AdmissionError>(source)
        })?;
    let completed = rig.completion(&mut job)?;
    assert!(matches!(completed.outcome, JobOutcome::Success(_)));
    drop(completed);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn registered_startup_cancel_at_original_clock_observation_prevents_issuance_and_body_copy()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let mut startup = request_snapshot(&rig, &boundary, &session)?;
    let profile = request()?.profile_directory().to_path_buf();
    let until = startup.launch.as_ref().ok_or("Original Entry missing")?.until();
    let client = rig.client.clone();
    let original = run()?;
    let clock = rig.clock.clone();
    let core = rig.client.core.clone();
    let scope = rig.scope.id();
    let mut job =
        rig.client.submit_blocking(&rig.scope, projection_spec(2, until)?, move |context| {
            clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(scope)));
            assert!(
                client.register_startup_source(&mut startup, &context, original, &profile).is_err()
            );
            assert!(context.memory().claim(BYTES).is_ok());
            assert!(
                !lock(&client.core.state)
                    .jobs
                    .get(&context.job_id())
                    .is_some_and(|record| record.registered_startup_issued)
            );
            Ok::<_, AdmissionError>(())
        })?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}

#[test]
fn registered_startup_queue_saturation_retains_exact_source_and_first_cutoff_without_issuance()
-> TestResult {
    use crate::runtime_test_support::Gate;
    struct Release(Arc<Gate>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.open();
        }
    }
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let entered_a = Arc::new(Gate::default());
    let entered_b = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = Release(release.clone());
    let mut active = Vec::new();
    for (id, class, entered) in [
        (3, JobClass::InfrastructureOrdinary, entered_a.clone()),
        (4, JobClass::InfrastructureLifecycle, entered_b.clone()),
    ] {
        let release = release.clone();
        active.push(rig.client.submit_blocking(&rig.scope, spec(id, class, 4096)?, move |_| {
            entered.open();
            release.wait().map_err(|cause| std::io::Error::other(cause.to_string()))
        })?);
    }
    entered_a.wait()?;
    entered_b.wait()?;
    let mut queued = Vec::new();
    for index in 0..32 {
        let class = if index < 28 {
            JobClass::InfrastructureOrdinary
        } else {
            JobClass::InfrastructureLifecycle
        };
        queued.push(rig.client.submit_blocking(
            &rig.scope,
            spec(100 + index, class, 4096)?,
            |_| Ok::<_, std::io::Error>(()),
        )?);
    }
    let mut input = Some(source.clone());
    let client = rig.client.clone();
    let previous = run()?;
    let new = next(previous)?;
    let rejected = rig.client.submit_blocking_owned_box(
        &rig.scope,
        projection_spec(400, original.until())?,
        &mut input,
        move |input, context| {
            client.restart_startup_snapshot(input.input(), previous, new, original, &context)
        },
    );
    assert!(matches!(
        rejected,
        Err(crate::OwnedInputError::Admission(AdmissionError::QueueSaturated { .. }))
    ));
    assert!(input.as_ref().is_some_and(|retained| source.same_source(retained)));
    assert_eq!(source.state()?.progress.lock().map_err(|_| "Progress poisoned")?.current, previous);
    rig.clock.advance(original.until().as_nanos() - rig.client.now()?.as_nanos());
    let client = rig.client.clone();
    let expired = rig.client.submit_blocking_owned_box(
        &rig.scope,
        projection_spec(400, original.until())?,
        &mut input,
        move |input, context| {
            client.restart_startup_snapshot(input.input(), previous, new, original, &context)
        },
    );
    assert!(matches!(
        expired,
        Err(crate::OwnedInputError::Admission(AdmissionError::DeadlineExpired { .. }))
    ));
    assert!(input.as_ref().is_some_and(|retained| source.same_source(retained)));
    for job in &queued {
        job.cancel();
    }
    release.open();
    for job in &mut active {
        drop(rig.completion(job)?);
    }
    for job in &mut queued {
        drop(rig.completion(job)?);
    }
    drop(input);
    drop(source);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}

#[test]
fn registered_startup_original_main_cancel_rejects_capsule_before_native_parent_effect()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let previous = run()?;
    let new = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            let capsule =
                client.restart_startup_snapshot(&retained, previous, new, original, &context)?;
            let binding = capsule
                .admission_binding()
                .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
            client.core.cancel_job(context.job_id());
            let now = client
                .now()
                .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
            assert!(
                binding
                    .validate_current_locked(&client.core, &lock(&client.core.state), new, now)
                    .is_err()
            );
            drop(binding);
            drop(capsule);
            Ok::<_, AdmissionError>(())
        },
    )?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    drop(source);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
