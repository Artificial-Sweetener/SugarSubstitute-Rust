// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject the cancelled original scope using the real driver factory and execution owner.
use super::*;
use std::sync::Arc;
use sugarsubstitute_desktop::MainCaptionLabels;
use sugarsubstitute_execution::ExecutionOwner;
struct Frozen;
impl Clock for Frozen {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
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
#[test]
fn actual_windows_owner_layout() {
    println!(
        "ProductionChild={} ProductionStartup={} ProfileTransaction={}",
        size_of::<ProductionChild>(),
        size_of::<crate::production::startup::ProductionStartup>(),
        size_of::<profile_transaction::ProfileTransaction>()
    );
}
#[test]
fn real_driver_factory_cancelled_scope_keeps_partial_custody_until_actual_native_absence()
-> Result<()> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let owner = ExecutionOwner::new(Arc::new(Frozen), Arc::new(Wake))?;
    let client = owner.client();
    let jobs = JobSequence::new();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    scope.cancel();
    let child = ProductionChild::new(ChildInputs {
        input: InheritedLifecycleBootstrap::new(1)?,
        original: LifecyclePipeDeadline::new(100, 100)?,
        run,
        native_scope: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        identity: (LaunchId::new(1)?, Generation::new(1)?),
        settings: MainWindowSettings {
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "minimize",
                maximize: "maximize",
                restore: "restore",
                close: "close",
            },
        },
    })?;
    let mut child = crate::production::startup::constructor_roundtrip(
        run,
        LaunchId::new(1)?,
        child,
        jobs.clone(),
    )?;
    assert!(child.start(&client, scope, &jobs).is_err());
    assert!(child.failed);
    assert_eq!(child.completion_intent(), CompletionIntent::StartupFailed);
    assert!(child.bootstrap.scope().ok_or("Actual scope lost")?.snapshot().cancellation_requested);
    assert!(child.bootstrap.native_scope().is_none());
    assert!(!child.bootstrap.capture_admitted());
    assert!(!child.bootstrap.channel_mut().admitted());
    assert!(child.take_prepared_startup().is_none());
    assert_eq!(jobs.allocate()?.get(), 1);
    assert!(child.begin_shutdown()?.is_none());
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    child.request_shutdown(budget)?;
    assert!(!child.poll_shutdown()?);
    child.observe(DesktopStartupObservation::NativeRetired(run), None)?;
    assert!(child.poll_shutdown()?);
    assert_eq!(child.shutdown_budget(), Some(budget));
    child.finish_shutdown()?;
    assert!(child.bootstrap.scope().is_some());
    drop(child);
    drop(owner);
    Ok(())
}
