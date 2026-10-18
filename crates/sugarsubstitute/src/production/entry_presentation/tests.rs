// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual Root presentation/execution owner while faking native submission and child facts.
use super::*;
use crate::production::{
    dispatch::ProductionCommand,
    entry_runner::{EntryResources, EntrySplashState},
    options::ProductionEntryOptions,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, ExecutionError, Generation, SurfaceId},
    lifecycle::{
        CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        ShutdownBudget,
    },
};
use sugarsubstitute_desktop::HeadlessStep;
use sugarsubstitute_execution::AuthenticatedEntryLaunch;
type TestResult = Result<(), Box<dyn Error>>;

#[path = "retirement_tests.rs"]
mod retirement_tests;

struct FrozenClock;
impl sugarsubstitute_contracts::execution::Clock for FrozenClock {
    fn now(&self) -> sugarsubstitute_contracts::execution::Moment {
        sugarsubstitute_contracts::execution::Moment::from_nanos(0)
    }
    fn sleep_until(
        &self,
        _: sugarsubstitute_contracts::execution::Moment,
    ) -> sugarsubstitute_contracts::execution::WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                sugarsubstitute_contracts::execution::Operation::Infrastructure,
                Box::new(std::io::Error::other("Presentation test wake poisoned")),
            )
        })?;
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn revision(&self) -> Result<u64, Box<dyn Error>> {
        Ok(*self.revision.lock().map_err(|_| "Test wake poisoned")?)
    }
    fn wait(&self, previous: u64, until: Instant) -> TestResult {
        let revision = self.revision.lock().map_err(|_| "Test wake poisoned")?;
        let (_guard, timeout) = self
            .changed
            .wait_timeout_while(
                revision,
                until.saturating_duration_since(Instant::now()),
                |current| *current == previous,
            )
            .map_err(|_| "Test wake poisoned")?;
        if timeout.timed_out() {
            Err("Presentation completion guard expired".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct Child {
    pending: Rc<Cell<Option<MainIdentity>>>,
    acknowledged: Rc<RefCell<Vec<MainIdentity>>>,
    dropped: Rc<Cell<bool>>,
}
impl Drop for Child {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}
impl EntryActivation for Child {
    fn entry_splash(&self) -> EntrySplashState {
        EntrySplashState { presentation: None, handoff: self.pending.get() }
    }
    fn complete_entry_splash_handoff(&mut self, identity: MainIdentity) -> TestResult {
        if self.pending.get() != Some(identity) {
            return Err("Foreign child handoff".into());
        }
        self.acknowledged.borrow_mut().push(identity);
        self.pending.set(None);
        Ok(())
    }
    fn enter_activation(
        &mut self,
        _: &mut Option<AuthenticatedEntryLaunch>,
        _: EntryResources<'_>,
    ) -> TestResult {
        Err("No fixture activation".into())
    }
    fn consume(
        &mut self,
        _: &CompletionBatch,
        _: EntryResources<'_>,
    ) -> Result<HeadlessStep, Box<dyn Error>> {
        Ok(HeadlessStep::Active)
    }
    fn request_close(&mut self, _: Option<ShutdownBudget>) -> TestResult {
        Ok(())
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        None
    }
    fn normal_close_requested(&self) -> bool {
        false
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn poll_close(
        &mut self,
        _: &CompletionBatch,
        resources: Option<EntryResources<'_>>,
        _: ShutdownBudget,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(resources.is_none())
    }
    fn finish(&mut self) -> TestResult {
        Ok(())
    }
}
fn content(child: Child) -> Result<EntryPresentation<Child>, Box<dyn Error>> {
    // Lexically invalid installation rejects before filesystem/environment work if entry begins.
    let path = PathBuf::from("unopened-relative-input");
    let command = ProductionCommand {
        installation: path.clone(),
        diagnostics: Default::default(),
        options: ProductionEntryOptions {
            configuration: path.clone(),
            package: path.clone(),
            metadata: path.clone(),
            cubes: path.clone(),
            script: path.clone(),
            profile: path.clone(),
        },
    };
    let mut entry = Box::new(EntryRunner::new(
        command,
        path,
        Box::new(child),
        crate::diagnostics::DiagnosticHealth::fixture(),
    ));
    entry.set_test_clock(Arc::new(FrozenClock))?;
    EntryPresentation::new(entry)
}
fn identity() -> Result<FrameIdentity, Box<dyn Error>> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    })
}
fn main_identity(generation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(generation)?,
        },
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn close(content: &mut EntryPresentation<Child>) -> TestResult {
    content.request_shutdown()?;
    let until = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Guard overflow")?;
    while !content.poll_shutdown()? {
        if Instant::now() >= until {
            return Err("Original entry execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    content.finish_shutdown()
}

#[test]
fn complete_real_frame_precedes_entry_budget_configuration_and_all_native_admission() -> TestResult
{
    let child = Child::default();
    let dropped = child.dropped.clone();
    let mut content = content(child)?;
    let wake = Arc::new(Wake::default());
    content.start(wake.clone())?;
    content.set_visible(true)?;
    content.request_frame(FrameShape::new(558, 558, 1.0)?, Duration::ZERO)?;
    assert!(!content.entry_ref()?.entry_setup_started());
    assert!(content.activity_deadline().is_none());
    let until = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Guard overflow")?;
    let frame = loop {
        let revision = wake.revision()?;
        if let Some(frame) = content.poll_frame(NonZeroUsize::MIN.saturating_add(31))? {
            break frame;
        }
        wake.wait(revision, until)?;
    };
    // Complete first presentation includes visible status-region pixels; bootstrap omits this region.
    assert!((486..534).any(|y| (24..534).any(|x| frame.pixels()[(y * 558 + x) * 4 + 3] != 0)));
    assert!(!content.entry_ref()?.entry_setup_started());
    let cause = match content.frame_presented(frame.identity()) {
        Err(cause) => cause,
        Ok(()) => return Err("Invalid installation unexpectedly admitted".into()),
    };
    let original = cause.as_ref() as *const dyn Error;
    assert!(content.first_presented);
    assert!(content.entry_ref()?.entry_setup_started());
    let original_deadline = content.activity_deadline();
    content.frame_presented(frame.identity())?;
    assert_eq!(content.activity_deadline(), original_deadline);
    close(&mut content)?;
    let failure = content.transfer_failure(cause);
    assert!(content.entry.is_none());
    assert!(content.preparation.is_none());
    assert!(content.close_requested());
    assert!(content.start(Arc::new(Wake::default())).is_err());
    let envelope = failure.as_ref() as *const dyn Error;
    let failure = report_failure(failure, |title, summary, details| {
        assert_eq!(title, sugarsubstitute::APPLICATION_NAME);
        assert_eq!(
            summary,
            sugarsubstitute_contracts::localization::messages("en").report.startup_failed
        );
        assert!(!format!("{details}").is_empty());
        Err(std::io::Error::other("Native dialog refused"))
    });
    assert!(std::ptr::eq(failure.as_ref() as *const dyn Error, envelope));
    assert!(!dropped.get());
    let mut current = Some(failure.as_ref());
    let mut retained = false;
    for _ in 0..16 {
        let Some(cause) = current else { break };
        retained |= std::ptr::eq(cause as *const dyn Error, original);
        current = cause.source();
    }
    assert!(retained);
    drop(failure);
    assert!(dropped.get());
    Ok(())
}

#[test]
fn absent_or_foreign_prepared_frame_cannot_start_entry_and_cancel_joins_without_admission()
-> TestResult {
    let mut content = content(Child::default())?;
    content.start(Arc::new(Wake::default()))?;
    assert!(content.frame_presented(identity()?).is_err());
    let mut foreign = identity()?;
    foreign.sequence = Generation::new(2)?;
    content.first_prepared = Some(foreign);
    assert!(content.frame_presented(identity()?).is_err());
    assert!(!content.entry_ref()?.entry_setup_started());
    close(&mut content)?;
    assert!(!content.entry_ref()?.entry_setup_started());
    Ok(())
}

#[test]
fn authenticated_handoff_requires_actual_hide_and_reuses_hidden_fact_for_later_children()
-> TestResult {
    let child = Child::default();
    let pending = child.pending.clone();
    let acknowledged = child.acknowledged.clone();
    let mut content = content(child)?;
    content.first_presented = true;
    pending.set(Some(main_identity(1)?));
    content.observe_handoff()?;
    assert!(!content.presentation_visible());
    assert!(acknowledged.borrow().is_empty());
    assert!(content.activity_deadline().is_none());
    content.presentation_hidden()?;
    assert_eq!(acknowledged.borrow().as_slice(), &[main_identity(1)?]);
    assert!(content.activity_deadline().is_some());
    content.presentation_hidden()?;
    assert_eq!(acknowledged.borrow().len(), 1);
    pending.set(Some(main_identity(2)?));
    content.observe_handoff()?;
    assert_eq!(acknowledged.borrow().as_slice(), &[main_identity(1)?, main_identity(2)?]);
    content.observe_handoff()?;
    assert_eq!(acknowledged.borrow().len(), 2);
    assert!(!content.presentation_visible());
    Ok(())
}

#[test]
fn changing_pending_handoff_or_cancelling_before_hide_cannot_authorize_child_show() -> TestResult {
    let child = Child::default();
    let pending = child.pending.clone();
    let acknowledged = child.acknowledged.clone();
    let mut content = content(child)?;
    content.first_presented = true;
    pending.set(Some(main_identity(1)?));
    content.observe_handoff()?;
    pending.set(Some(main_identity(2)?));
    assert!(content.observe_handoff().is_err());
    assert!(acknowledged.borrow().is_empty());
    content.closing = true;
    content.presentation_hidden()?;
    assert!(acknowledged.borrow().is_empty());
    Ok(())
}
