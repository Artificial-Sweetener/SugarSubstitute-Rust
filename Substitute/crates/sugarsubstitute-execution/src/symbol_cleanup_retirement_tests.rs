// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve held factory custody through shutdown, destruction failure and fatal clock rejection.
use super::*;

#[test]
fn shutdown_keeps_actual_held_factory_and_bytes_until_supervised_destruction_returns() -> TestResult
{
    let mut rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release_on_failure = ReleaseGate(release.clone());
    let dropped = Arc::new(AtomicUsize::new(0));
    let captured = HeldDestruction {
        entered: entered.clone(),
        release: release.clone(),
        dropped: dropped.clone(),
    };
    let effects = Arc::new(AtomicUsize::new(0));
    let called = effects.clone();
    let mut permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        move |_| {
            called.fetch_add(1, Ordering::AcqRel);
            drop(captured);
            Ok::<_, io::Error>(())
        },
    ))?;
    rig.owner.request_shutdown(Moment::from_nanos(1));
    entered.wait()?;
    assert_eq!(permit.status().phase, JobPhase::Running);
    assert!(permit.try_take_completion().is_none());
    assert_eq!(cleanup.snapshot().reserved_bytes, 32_768);
    assert_eq!(effects.load(Ordering::Acquire), 0);
    assert_eq!(rig.owner.poll_shutdown()?.unresolved_job_ids, vec![cleanup_spec.id]);
    release.open();
    let mut actual = permit.cancel_and_observe();
    let completion = rig.completion(&mut actual)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    drop(completion);
    drop(actual);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}

struct DestructionPanic(Arc<AtomicUsize>);
impl Drop for DestructionPanic {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
        std::panic::resume_unwind(Box::new("held cleanup destructor".to_owned()));
    }
}
#[test]
fn held_factory_destruction_panic_is_actual_terminal_cause_before_byte_release() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let dropped = Arc::new(AtomicUsize::new(0));
    let captured = DestructionPanic(dropped.clone());
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        move |_| {
            drop(captured);
            Ok::<_, io::Error>(())
        },
    ))?;
    let mut actual = permit.cancel_and_observe();
    let completion = rig.completion(&mut actual)?;
    assert!(
        matches!(&completion.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "held cleanup destructor")
    );
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    assert_eq!(cleanup.snapshot().reserved_bytes, 32_768);
    drop(completion);
    drop(actual);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn fatal_clock_before_main_admission_preserves_original_cause_and_owned_factory() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        |_| Ok::<_, io::Error>(()),
    ))?;
    let effects = Arc::new(AtomicUsize::new(0));
    let called = effects.clone();
    rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new("original symbol clock cause".to_owned()));
    }));
    let rejected =
        match rig.client.submit_symbol_helper(&rig.scope, main_spec, &permit, move |_| {
            called.fetch_add(1, Ordering::AcqRel);
            Ok::<_, io::Error>(())
        }) {
            Err(rejected) => rejected,
            Ok(_) => return Err("Fatal clock admitted symbol main".into()),
        };
    let AdmissionError::Unexpected(cause) = rejected.error else {
        return Err("Fatal clock cause replaced by admission classification".into());
    };
    assert!(
        std::error::Error::source(&cause)
            .is_some_and(|cause| cause.to_string().contains("original symbol clock cause"))
    );
    assert_eq!(effects.load(Ordering::Acquire), 0);
    assert_eq!(permit.original_job_id(), cleanup_spec.id);
    drop(rejected.factory);
    let actual = permit.cancel_and_observe();
    drop(rig.owner);
    assert_eq!(
        actual.status().phase,
        JobPhase::Terminal(sugarsubstitute_contracts::execution::TerminalKind::Cancelled)
    );
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
