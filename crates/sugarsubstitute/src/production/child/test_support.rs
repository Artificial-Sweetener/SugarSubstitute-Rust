// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Count external driver calls without fabricating authenticated inputs.
use super::{ChildBootstrap, MainWindowSettings, ProductionChild, capture, channel};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionError, Generation, Moment, ScopeId, ScopeSpec, WaitFuture,
    },
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};
use sugarsubstitute_execution::{
    CaptureBundleAdmission, CaptureBundleRetirement, NativeCaptureDriver, NativeCaptureInput,
    NativeCaptureProgress, NativeCaptureReply, NativeCaptureStep, windows::NativeObservationError,
};
pub(super) struct InertDriver(pub(super) Arc<AtomicUsize>);
impl NativeCaptureDriver for InertDriver {
    fn prepare(&mut self, _: CaptureBundleAdmission) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn attach(
        &self,
        _: &NativeCaptureInput,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn poll(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn issue_step(
        &self,
        _: &NativeCaptureStep,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire_prepared(
        &mut self,
        _: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(false)
    }
    fn retained_pin_bytes(&self) -> u64 {
        0
    }
}

/// Original deterministic identity shared by the real child fixture and composed callback tests.
pub(in crate::production) fn run() -> super::Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}

/// Frozen execution clock used by actual child/recipient lifecycle tests.
pub(in crate::production) struct Frozen(pub(in crate::production) AtomicU64);
impl Clock for Frozen {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        Ok(())
    }
}
/// Construct the real child with its real execution owner; only capture and time are external fakes.
pub(in crate::production) fn child()
-> super::Result<(ProductionChild, sugarsubstitute_execution::ExecutionOwner, Arc<Frozen>)> {
    let run = run()?;
    let clock = Arc::new(Frozen(AtomicU64::new(0)));
    let owner = sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let identity = (LaunchId::new(1)?, Generation::new(1)?);
    let bootstrap = ChildBootstrap::new(
        channel::ChildChannel::new(
            InheritedLifecycleBootstrap::new(1)?,
            LifecyclePipeDeadline::new(100, 100)?,
            run,
        ),
        capture::ChildCapture::new(Box::new(InertDriver(Arc::default()))),
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        identity,
    )?;
    let mut child = ProductionChild::assemble(
        bootstrap,
        run,
        identity,
        MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: sugarsubstitute_desktop::MainCaptionLabels {
                minimize: "min",
                maximize: "max",
                restore: "restore",
                close: "close",
            },
        },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    child.client = Some(owner.client());
    Ok((child, owner, clock))
}
