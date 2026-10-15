// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise finite registration and persistent custody through the actual admitted worker.
use super::{registered::RegisteredDriver, *};
use crate::{
    admission::Core,
    runtime_test_support::{ClockPort, ManualClock, Wake},
    task::{ErasedJob, Work},
};

struct RegisteredOwner {
    core: Arc<Core>,
    clock: Arc<ManualClock>,
    scope: Scope,
    parent: Scope,
    boundary: Arc<Boundary>,
    session: DesktopCaptureSession,
    active: Box<worker::DesktopCaptureActive>,
    metadata: metadata::ChildMetadata,
    queued_job: Arc<dyn ErasedJob>,
}
impl RegisteredOwner {
    fn new(parent_deadline: Option<Moment>) -> TestResult<Self> {
        let inherited = input()?;
        #[cfg(feature = "k1-native-qualification")]
        let inherited = {
            let mut original = inherited;
            original.reserve_preview_control_storage(8192)?;
            original
        };
        Self::with_input(parent_deadline, inherited)
    }
    fn with_input(
        parent_deadline: Option<Moment>,
        inherited: InheritedCaptureAttachInput,
    ) -> TestResult<Self> {
        let clock = Arc::new(ManualClock::default());
        let core = Core::new(
            ExecutionId::new(1)?,
            Arc::new(ClockPort(clock.clone())),
            Arc::new(Wake::default()),
        );
        let client = crate::ExecutionClient { core: core.clone() };
        let parent = client
            .create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: parent_deadline }, None)?;
        let scope = client
            .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&parent))?;
        let boundary = Arc::new(Boundary::default());
        let driver = RegisteredDriver(Driver {
            boundary: boundary.clone(),
            blocked: false,
            claims: Vec::new(),
        });

        let session =
            submission::reserve(&core, &scope, request()?, inherited, Box::new(driver), event)?;
        let (queued, wake) =
            core.take_native_at(Moment::from_nanos(0)).ok_or("Actual capture not selected")?;
        core.deliver_capacity_wake(wake);
        let Work::Native(crate::native::NativeWork::DesktopCapture(work)) = queued.work else {
            return Err("Capture routed to finite factory".into());
        };
        let active =
            worker::DesktopCaptureActive::new(*work, queued.context, &core, Moment::from_nanos(0));
        let metadata = active.metadata();
        Ok(Self {
            core,
            clock,
            scope,
            parent,
            boundary,
            session,
            active,
            metadata,
            queued_job: queued.job,
        })
    }
    fn register(&mut self) -> TestResult {
        self.active.progress(&self.core, Moment::from_nanos(0), true);
        let completion =
            self.session.job().try_take_completion().ok_or("Genuine Registered result missing")?;
        assert!(matches!(completion.outcome, JobOutcome::Success(_)));
        assert_eq!(completion.memory.bytes(), 4_194_304);
        drop(completion);
        Ok(())
    }
    fn retained(&self) -> TestResult {
        assert!(!self.active.completed());
        assert_eq!(
            self.session.control().temporary_settled(),
            DesktopCaptureTemporaryState::Pending
        );
        assert_eq!(self.scope.snapshot().reserved_bytes, 4_194_304);
        assert_eq!(self.boundary.retired.load(Ordering::Acquire), 0);
        assert_eq!(
            self.active.event.as_ref().ok_or("Actual Event owner missing")?.original.snapshot(),
            (2, 2)
        );
        assert!(self.active.work.inherited.is_some());
        assert_eq!(
            lock(&self.active.work.shared.state).phase,
            control::DesktopCapturePhase::Registered
        );
        assert!(!self.active.work.shared.cleanup.stopped());
        Ok(())
    }
    fn settle(mut self) -> TestResult {
        let original = self.session.control().cleanup_snapshot();
        let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
            budget,
            counter: Ok(before),
        } = original
        else {
            return Err("Original cleanup not latched".into());
        };
        assert_eq!(budget.graceful_until.checked_duration_since(budget.started)?.as_secs(), 10);
        assert_eq!(budget.settle_until.checked_duration_since(budget.started)?.as_secs(), 15);
        self.active.progress(&self.core, budget.started, true);
        assert!(self.active.completed());
        assert_eq!(
            self.session.control().temporary_settled(),
            DesktopCaptureTemporaryState::Transferred
        );
        assert_eq!(self.boundary.retired.load(Ordering::Acquire), 1);
        let drained = self.core.mailbox.drain(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        assert!(drained.notifications.is_empty());
        assert_eq!(drained.remaining, 0);
        assert_eq!(
            self.active.event.as_ref().ok_or("Checked Event owner missing")?.original.snapshot(),
            (0, 2)
        );
        assert!(self.active.work.inherited.is_none());
        let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
            counter: Ok(after),
            ..
        } = self.session.control().cleanup_snapshot()
        else {
            return Err("Original cleanup changed".into());
        };
        assert_eq!(before, after);
        let mut permanent = self
            .session
            .control()
            .try_take_process_lifetime()
            .ok_or("Actual permanent transfer missing")?
            .map_err(|cause| format!("{cause:?}"))?;
        assert_eq!(permanent.reservation_bytes(), 4_194_304);
        assert!(!lock(&self.active.work.shared.cleanup.state).native_retired);
        assert!(!lock(&self.active.work.shared.cleanup.state).source_retired);
        drop(self.active);
        drop(self.metadata);
        drop(self.queued_job);
        drop(self.session);
        drop(lock(&self.boundary.context).take());
        assert_eq!(self.scope.snapshot().reserved_bytes, 4_194_304);
        // This boundary owns no OS outputs. Destroy only its genuinely transferred
        // private fake capsule; never manufacture a public process-exit/retirement proof.
        let capsule = permanent.capsule.take().ok_or("Original fake capsule missing")?;
        drop(permanent);
        drop(capsule);
        assert_eq!(self.scope.snapshot().reserved_bytes, 0);
        Ok(())
    }
}

#[test]
fn registered_capture_outlives_finite_cutoff_then_actual_scope_cancel_settles_temporary_custody()
-> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    owner.clock.advance(100_000_000_001);
    owner.active.progress(&owner.core, Moment::from_nanos(100_000_000_001), true);
    owner.retained()?;
    assert_eq!(owner.active.context.deadline(), Some(Moment::from_nanos(100_000_000_000)));
    assert_eq!(owner.boundary.attaches.load(Ordering::Acquire), 1);
    assert!(owner.session.job().try_take_completion().is_none());
    owner.scope.cancel();
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    assert_eq!(owner.boundary.retired.load(Ordering::Acquire), 0);
    owner.settle()
}

/// A genuine retained native error cannot be mistaken for clean registered settlement.
#[test]
fn registered_cleanup_retains_exact_external_failure_in_mailbox_until_observed() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    owner.clock.advance(211_000_000_000);
    owner.active.progress(&owner.core, Moment::from_nanos(211_000_000_000), true);
    owner.retained()?;
    let original = NativeObservationError::new(
        "registered_retire_boundary",
        std::io::Error::from_raw_os_error(5),
    );
    *lock(&owner.boundary.retire_failure) = Some(original.clone());
    owner.session.control().request_cleanup();
    let initial = owner.session.control().cleanup_snapshot();
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
        budget,
        counter: Ok(before),
    } = initial
    else {
        return Err("Actual original cleanup transaction missing".into());
    };
    owner.active.progress(&owner.core, budget.started, true);
    assert!(!owner.active.completed());
    assert_eq!(owner.session.control().temporary_settled(), DesktopCaptureTemporaryState::Pending);
    assert_eq!(owner.boundary.retired.load(Ordering::Acquire), 0);
    assert!(owner.active.work.inherited.is_some());
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    let observed = owner
        .core
        .mailbox
        .drain(std::num::NonZeroUsize::MIN.saturating_add(31))
        .err()
        .ok_or("Actual external cleanup failure was not retained")?;
    assert_eq!(observed.operation(), owner.active.context.spec().operation);
    // The known execution-owned aggregate and its first typed error are inspected
    // at this owner; the controlled external error's callbacks are never searched.
    let aggregate = std::error::Error::source(&observed).ok_or("Aggregate lost")?;
    let first = aggregate
        .source()
        .and_then(|cause| cause.downcast_ref::<ExecutionError>())
        .ok_or("Original execution failure lost")?;
    let retained = std::error::Error::source(first)
        .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
        .ok_or("Original native failure lost")?;
    assert_eq!(retained.stage(), original.stage());
    assert_eq!(retained.win32_code(), Some(5));
    assert!(std::ptr::eq(
        std::error::Error::source(retained).ok_or("Retained leaf lost")?,
        std::error::Error::source(&original).ok_or("Original leaf lost")?,
    ));
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
        budget: after_budget,
        counter: Ok(after),
    } = owner.session.control().cleanup_snapshot()
    else {
        return Err("Actual cleanup transaction replaced".into());
    };
    assert_eq!(after_budget, budget);
    assert_eq!(after, before);
    // Only this test's external boundary changes. The same worker resumes with
    // its retained outputs and original deadline; observing an error settles none.
    *lock(&owner.boundary.retire_failure) = None;
    owner.settle()
}

#[test]
fn registered_capture_still_observes_actual_ancestor_scope_deadline() -> TestResult {
    let cutoff = Moment::from_nanos(75_000_000_000);
    let mut owner = RegisteredOwner::new(Some(cutoff))?;
    owner.register()?;
    owner.clock.advance(cutoff.as_nanos());
    owner.active.progress(&owner.core, cutoff, true);
    assert!(owner.active.work.shared.cleanup.stopped());
    assert!(matches!(
        lock(&owner.active.work.shared.state).stopped_outcome,
        Some(crate::task::Stop::TimedOut)
    ));
    owner.settle()
}

#[test]
fn registered_capture_still_observes_actual_ancestor_close_and_owner_shutdown() -> TestResult {
    for shutdown in [false, true] {
        let mut owner = RegisteredOwner::new(None)?;
        owner.register()?;
        owner.retained()?;
        if shutdown {
            owner.core.shutdown();
        } else {
            owner.core.close_scope(owner.parent.id());
        }
        owner.active.progress(&owner.core, Moment::from_nanos(0), true);
        assert!(owner.active.work.shared.cleanup.stopped());
        owner.settle()?;
    }
    Ok(())
}

#[test]
fn finite_registration_cutoff_still_vetoes_unissued_capture() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.clock.advance(100_000_000_000);
    owner.active.progress(&owner.core, Moment::from_nanos(100_000_000_000), true);
    let completion = owner.session.job().try_take_completion().ok_or("Original timeout missing")?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert!(!lock(&owner.active.work.shared.state).registered_published);
    assert_eq!(owner.boundary.attaches.load(Ordering::Acquire), 0);
    assert_eq!(owner.boundary.prepares.load(Ordering::Acquire), 0);
    assert!(owner.active.completed());
    drop(completion);
    drop(owner.active);
    drop(owner.metadata);
    drop(owner.queued_job);
    drop(owner.session);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[cfg(feature = "k1-native-qualification")]
#[path = "../../registered_desktop_fault/capture_tests.rs"]
mod qualification;

#[cfg(feature = "k1-native-qualification")]
#[path = "../../preview_arming/capture_tests.rs"]
mod preview;
