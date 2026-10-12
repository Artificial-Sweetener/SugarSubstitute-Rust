// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Hold finite external open after actual admission and prove cancellation remains available.
use super::*;
use crate::scope::lock;
use crate::{
    runtime_test_support::{Gate, TestResult, spec},
    symbol_test_support::LiveSymbol,
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{JobClass, JobOutcome, Operation, ScopeId, ScopeSpec};
struct Boundary {
    veto_deadline: Mutex<Option<Moment>>,
    accepted: Arc<Gate>,
    release: Arc<Gate>,
    effects: AtomicUsize,
    modes: AtomicUsize,
    thread: Mutex<Option<std::thread::ThreadId>>,
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
struct FiniteLease {
    inner: Lease,
    boundary: Arc<Boundary>,
    binding: PeerBinding,
}
impl PipeLease for FiniteLease {
    fn adopt(&mut self, _: PeerInput) -> Result<(), (Box<LifecyclePipeFailure>, PeerInput)> {
        Ok(())
    }
    fn prepare(
        &mut self,
        _: [Arc<Signal>; 3],
        context: &JobContext,
        _: Moment,
    ) -> Result<usize, LifecyclePipeFailure> {
        self.inner.bind_effect_context(context);
        self.inner.data = Some(bootstrap::Data {
            binding: self.binding,
            pid: 1,
            birth: 1,
            session: 1,
            deadline: LifecyclePipeDeadline::new(1000, 1000)
                .map_err(|cause| error("fixture_deadline", cause))?,
            capability: [7; 32],
            name: vec![0],
        });
        Ok(0)
    }
    fn binding(&self) -> Option<PeerBinding> {
        Some(self.binding)
    }
    fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        LifecyclePipeDeadline::new(1000, 1000).ok()
    }
    fn capability(&self) -> Option<[u8; 32]> {
        Some([7; 32])
    }
    fn connect(&mut self, issue: Issue<'_>) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        match issue.execute(|| self.inner.claim_client_open())? {
            Dispatch::Issued(stage) => {
                let boundary = self.boundary.clone();
                let opened = stage.run(|_| {
                    *lock(&boundary.thread) = Some(std::thread::current().id());
                    boundary.accepted.open();
                    boundary.release.wait().map_err(|_| {
                        windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
                            ERROR_GEN_FAILURE.0,
                        ))
                    })?;
                    boundary.effects.fetch_add(1, Ordering::AcqRel);
                    // SAFETY: fake external pipe-open returns an actual private event to the production retained Handle owner.
                    unsafe { CreateEventW(None, true, false, None) }
                });
                if let Err(cause) = opened {
                    if let Some(sugarsubstitute_contracts::execution::AdmissionError::DeadlineExpired { deadline, .. }) = std::error::Error::source(cause.native_cause()).and_then(|source| source.downcast_ref::<sugarsubstitute_contracts::execution::AdmissionError>()) {
                        *lock(&self.boundary.veto_deadline) = Some(*deadline);
                    }
                    return Err(cause);
                }
                self.boundary.modes.fetch_add(1, Ordering::AcqRel);
                Ok(Dispatch::Issued(Progress::Complete(0)))
            }
            Dispatch::Revoked => Ok(Dispatch::Revoked),
            Dispatch::Stopped => Ok(Dispatch::Stopped),
            Dispatch::Expired => Ok(Dispatch::Expired),
        }
    }
    fn read(
        &mut self,
        _: &mut [u8],
        _: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        Err(failure("unexpected_fixture_read"))
    }
    fn write(
        &mut self,
        _: &[u8],
        _: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        Err(failure("unexpected_fixture_write"))
    }
    fn poll(&mut self, _: Role) -> Result<Progress, LifecyclePipeFailure> {
        Err(failure("unexpected_fixture_poll"))
    }
    fn quiesce(&mut self, _: Role) -> Result<(), LifecyclePipeFailure> {
        Ok(())
    }
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure> {
        Err(failure("unexpected_fixture_verify"))
    }
    fn authenticate(&mut self, _: &[u8; 32]) -> Result<VerifiedPeer, LifecyclePipeFailure> {
        Err(failure("unexpected_fixture_auth"))
    }
    fn take_completed_write(&mut self) -> Option<u32> {
        None
    }
    fn retire(&mut self, _: bool) -> Result<bool, LifecyclePipeFailure> {
        self.inner.pipe.close("fixture_checked_pipe_close")?;
        Ok(true)
    }
}
fn binding() -> TestResult<PeerBinding> {
    use sugarsubstitute_contracts::{incident::*, lifecycle::*};
    Ok(PeerBinding::Diagnostic(DiagnosticKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        DiagnosticRole::Symbolize,
        DiagnosticSessionId::new(1)?,
    )))
}
enum StopAtBoundary {
    MainCancel,
    ChildCancel,
    MainExpiry,
    ChildExpiry,
}
fn stopped_open(action: StopAtBoundary) -> TestResult {
    let live = LiveSymbol::new()?;
    let resources = live.transport()?;
    let claim = resources.handles.clone();
    let scope = live.rig.client.create_native_scope(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&live.rig.scope),
    )?;
    let mut specification = spec(3, JobClass::NativeObservation, 196_608)?;
    specification.operation = Operation::NativeLifecycle;
    let boundary = Arc::new(Boundary {
        veto_deadline: Mutex::new(None),
        accepted: Arc::new(Gate::default()),
        release: Arc::new(Gate::default()),
        effects: AtomicUsize::new(0),
        modes: AtomicUsize::new(0),
        thread: Mutex::new(None),
    });
    let _release_on_failure = Release(boundary.release.clone());
    let held = boundary.clone();
    let binding = binding()?;
    let kind = crate::admission::RequestKind::SymbolNative {
        original: resources.original.as_ref().clone(),
        handles: resources.handles.clone(),
    };
    let native_deadline = LifecyclePipeDeadline::new(1000, 1000)?;
    let (control, mut job) = crate::lifecycle_pipe::admit_owned(
        &live.rig.client.core,
        &scope,
        specification,
        crate::lifecycle_pipe::PipeAdmission {
            binding: None,
            handshake: Moment::from_nanos(9_000_000_000),
            owner: Owner::Diagnostic,
        },
        kind,
        resources,
        |resources| {
            Box::new(FiniteLease {
                inner: Lease::diagnostic_client(
                    DiagnosticRole::Symbolize,
                    native_deadline,
                    Some(resources),
                ),
                boundary: held,
                binding,
            })
        },
    )
    .map_err(|(cause, _)| cause)?;
    control.bind_input(PeerInput::Simulated);
    boundary.accepted.wait()?;
    assert_eq!(live.cleanup_scope.snapshot().reserved_bytes, 32_768);
    assert_eq!(lock(&live.rig.client.core.state).bytes, 8_388_608);
    assert_ne!(*lock(&boundary.thread), Some(std::thread::current().id()));
    assert_eq!(scope.snapshot().reserved_bytes, 196_608);
    assert_eq!(live.rig.scope.snapshot().reserved_bytes, 8_159_232);
    assert_eq!(claim.in_custody(), 1);
    let timed_out = match action {
        StopAtBoundary::MainCancel => {
            live.rig.scope.cancel();
            assert!(live.rig.scope.snapshot().cancellation_requested);
            false
        }
        StopAtBoundary::ChildCancel => {
            scope.cancel();
            assert!(!live.context.cancellation_requested());
            false
        }
        StopAtBoundary::MainExpiry => {
            live.rig.clock.advance(10_000_000_000);
            true
        }
        StopAtBoundary::ChildExpiry => {
            live.rig.clock.advance(9_000_000_000);
            assert!(!live.context.cancellation_requested());
            true
        }
    };
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.effects.load(Ordering::Acquire), 0);
    boundary.release.open();
    let completion = live.rig.completion(&mut job)?;
    assert!(if timed_out {
        matches!(completion.outcome, JobOutcome::TimedOut)
    } else {
        matches!(completion.outcome, JobOutcome::Cancelled)
    });
    let expected_deadline = match action {
        StopAtBoundary::MainExpiry => Some(Moment::from_nanos(10_000_000_000)),
        StopAtBoundary::ChildExpiry => Some(Moment::from_nanos(9_000_000_000)),
        _ => None,
    };
    assert_eq!(*lock(&boundary.veto_deadline), expected_deadline);
    assert_eq!(boundary.effects.load(Ordering::Acquire), 1);
    assert_eq!(boundary.modes.load(Ordering::Acquire), 0);
    assert_eq!(claim.in_custody(), 0);
    assert!(!lock(&control.shared.state).connected);
    Ok(())
}

#[test]
fn symbol_blocked_accepted_finite_open_keeps_core_cancel_available_and_exact_custody() -> TestResult
{
    stopped_open(StopAtBoundary::MainCancel)
}
#[test]
fn symbol_child_cancel_after_finite_stage_keeps_original_main_live_and_vetoes_mode() -> TestResult {
    stopped_open(StopAtBoundary::ChildCancel)
}
#[test]
fn symbol_original_main_expiry_after_finite_stage_preserves_original_timeout_cause() -> TestResult {
    stopped_open(StopAtBoundary::MainExpiry)
}
#[test]
fn symbol_child_handshake_expiry_after_finite_stage_preserves_its_actual_timeout() -> TestResult {
    stopped_open(StopAtBoundary::ChildExpiry)
}
