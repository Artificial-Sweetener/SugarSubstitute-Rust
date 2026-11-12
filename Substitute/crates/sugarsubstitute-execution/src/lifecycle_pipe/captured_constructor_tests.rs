// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the production captured backend constructor with a genuine admitted external capture.
use super::*;
use crate::runtime_test_support::{ClockPort, Gate, ManualClock, Wake};
use std::sync::atomic::AtomicBool;
use sugarsubstitute_contracts::{crash::CaptureTarget, execution::Clock};

/// Hold the original native owner's dequeue boundary while its real constructor admits work.
struct DispatchClock {
    manual: Arc<ManualClock>,
    hold: AtomicBool,
    entered: Gate,
    release: Gate,
}
impl Clock for DispatchClock {
    fn now(&self) -> Moment {
        let observed = ClockPort(self.manual.clone()).now();
        if std::thread::current().name() == Some("sugarsubstitute-native-retirement")
            && self.hold.swap(false, Ordering::AcqRel)
        {
            self.entered.open();
            if let Err(cause) = self.release.wait() {
                std::panic::resume_unwind(Box::new(cause.to_string()));
            }
        }
        observed
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.manual.clone()).sleep_until(deadline)
    }
}
struct Release(Arc<DispatchClock>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release.open();
    }
}
fn rig() -> TestResult<(Rig, Arc<DispatchClock>)> {
    let manual = Arc::new(ManualClock::default());
    let clock = Arc::new(DispatchClock {
        manual: manual.clone(),
        hold: AtomicBool::new(false),
        entered: Gate::default(),
        release: Gate::default(),
    });
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::with_cpu_workers(clock.clone(), wake.clone(), 2)?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    Ok((Rig { owner, client, clock: manual, wake, scope }, clock))
}

#[test]
fn actual_captured_backend_server_installs_section_before_once_only_pin_transfer() -> TestResult {
    use crate::windows_backend_custody::CapturedBackendCustodyStart;
    let (rig, clock) = rig()?;
    let _release_dispatch = Release(clock.clone());
    let scope = scope(&rig)?;
    let target = CaptureTarget::BackendCustody(key()?);
    let mut facts_job = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        |_| creator_facts(),
    )?;
    let facts_completion = rig.completion(&mut facts_job)?;
    let creator = match &facts_completion.outcome {
        JobOutcome::Success(facts) => *facts,
        JobOutcome::Failure(cause) => return Err(cause.to_string().into()),
        _ => return Err("Original creator observation failed".into()),
    };
    let (_capture_scope, capture_job, pin, release_capture) =
        crate::capture_attach::test_support::admitted_pin_for_creator(&rig, target, creator)?;
    let mut pin = Some(pin);
    let alien = BackendCustodyKey::new(key()?.owner(), BackendCustodySession::new(10)?);
    assert!(matches!(
        CapturedBackendCustodyStart::new(alien, &mut pin),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert!(pin.is_some());
    let mut start = CapturedBackendCustodyStart::new(key()?, &mut pin)?;
    assert!(pin.is_none());
    let mut request = spec(6, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    let before = scope.snapshot();
    let rejected = rig.client.reserve_captured_backend_custody_pipe_server(
        &scope,
        request,
        key()?,
        Moment::from_nanos(0),
        &mut start,
    );
    assert!(matches!(rejected, Err(AdmissionError::DeadlineExpired { .. })));
    assert!(!start.is_consumed());
    assert_eq!(scope.snapshot(), before);
    clock.hold.store(true, Ordering::Release);
    let mut session = rig.client.reserve_captured_backend_custody_pipe_server(
        &scope,
        request,
        key()?,
        Moment::from_nanos(10_000_000_000),
        &mut start,
    )?;
    assert!(start.is_consumed());
    assert!(session.control.inner.shared.captured);
    clock.entered.wait()?;
    clock.release.open();
    prepared(&rig, &mut session)?;
    let (bootstrap, enrollment) =
        session.take_bootstrap()?.ok_or("Actual captured backend bootstrap missing")?;
    assert!(bootstrap.captures(key()?));
    assert!(bootstrap.matches_enrollment(&enrollment));
    let view = bootstrap.borrow_inheritance();
    use std::os::windows::io::AsRawHandle;
    let section = view.section.as_raw_handle() as usize;
    assert_ne!(section, 0);
    assert_eq!(
        view.capture_processes
            .ok_or("Actual captured backend lost process pins")?
            .map(|handle| handle.as_raw_handle() as usize),
        [41, 42]
    );
    session.control.close();
    drop((bootstrap, enrollment));
    drop(release_capture);
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    drop((session, capture_job));
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Query the current-process pseudo handle only on the original admitted execution job.
#[allow(unsafe_code)]
fn creator_facts()
-> Result<crate::windows_capture_attach::CaptureProcessFacts, crate::native::NativeObservationError>
{
    // SAFETY: the current-process pseudo handle remains valid during this finite query and is never adopted or closed.
    let handle = unsafe { windows::Win32::System::Threading::GetCurrentProcess() };
    let (pid, birth) = crate::native_pipe::security::process_facts(
        handle,
        &crate::native_pipe::handles::Handle::new(),
    )
    .map_err(|cause| cause.cause)?;
    crate::windows_capture_attach::CaptureProcessFacts::new(pid, birth).map_err(|cause| {
        crate::native::NativeObservationError::new("original_creator_facts", cause)
    })
}

/// Observe actual Prepared or the unchanged native terminal; never lose a failing completion to a wake timeout.
fn prepared(rig: &Rig, session: &mut BackendCustodyPipeSession) -> TestResult {
    loop {
        let revision = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if session.drain(NonZeroUsize::MAX).into_iter().any(|value| matches!(value, Fact::Prepared))
        {
            return Ok(());
        }
        if let Some(done) = session.take_terminal() {
            return match &done.outcome {
                JobOutcome::Failure(cause) => {
                    Err(format!("Actual captured preparation failed: {cause:?}").into())
                }
                other => {
                    Err(format!("Actual captured preparation returned {:?}", other.kind()).into())
                }
            };
        }
        rig.wake.wait_after(revision)?;
    }
}
