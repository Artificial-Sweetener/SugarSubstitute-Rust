// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake the external content boundary while retaining actual desktop preparation policy.
use crate::{
    AccessibleControl, DesktopStartupCommand, HitTarget, KeyAction, Pointer, PointerAction,
    WindowContent,
};
use crate::{DesktopStartupObservation, StartupPreparationStep, StartupWindowContent};
use std::{
    collections::VecDeque,
    error::Error,
    time::{Duration, Instant},
};
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_contracts::{
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    presentation::{FrameShape, PreparedFrame},
    startup::LaunchId,
};
pub(super) type TestResult = Result<(), Box<dyn Error>>;
pub(super) struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        Ok(())
    }
}
pub(super) struct Content {
    pub(super) run: RunKey,
    pub(super) launch: LaunchId,
    pub(super) calls: Vec<&'static str>,
    pub(super) steps: VecDeque<Result<StartupPreparationStep, Box<dyn Error>>>,
    pub(super) settlements: VecDeque<Result<bool, Box<dyn Error>>>,
    pub(super) start_failure: bool,
    pub(super) close_failure: bool,
    pub(super) budgets: Vec<usize>,
    pub(super) ready: std::rc::Rc<std::cell::Cell<bool>>,
    wake: Option<Arc<dyn CompletionWake>>,
}
impl Content {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            calls: Vec::new(),
            steps: VecDeque::new(),
            settlements: VecDeque::new(),
            start_failure: false,
            close_failure: false,
            budgets: Vec::new(),
            ready: std::rc::Rc::new(std::cell::Cell::new(false)),
            wake: None,
        })
    }
}
impl WindowContent for Content {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.calls.push("start");
        self.wake = Some(wake);
        if self.start_failure {
            return Err(std::io::Error::other("original authentication cause").into());
        }
        Ok(())
    }
    fn request_frame(&mut self, _: FrameShape, _: Duration) -> Result<(), Box<dyn Error>> {
        self.calls.push("frame");
        Ok(())
    }
    fn poll_frame(&mut self, _: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        self.calls.push("frame_poll");
        Ok(None)
    }
    fn set_visible(&mut self, _: bool) -> Result<(), Box<dyn Error>> {
        self.calls.push("visibility");
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.push("close");
        if self.close_failure {
            return Err(std::io::Error::other("later close cause").into());
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.calls.push("settle");
        self.settlements.pop_front().unwrap_or(Ok(false))
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.push("join");
        Ok(())
    }
    fn hit_test(&self, _: Pointer) -> HitTarget {
        HitTarget::Client
    }
    fn pointer(&mut self, _: PointerAction, _: Pointer) -> bool {
        false
    }
    fn key(&mut self, _: KeyAction) -> bool {
        false
    }
    fn animating(&self) -> bool {
        false
    }
    fn console(&self) -> Option<crate::ConsoleView<'_>> {
        None
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &[]
    }
    fn activate(&mut self, _: u16) -> bool {
        false
    }
}
impl StartupWindowContent for Content {
    fn prepare_native_ui(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>> {
        self.calls.push("prepare");
        self.budgets.push(maximum.get());
        let step = self.steps.pop_front().unwrap_or(Ok(StartupPreparationStep::Waiting));
        if matches!(step, Ok(StartupPreparationStep::Ready)) {
            self.ready.set(true);
        }
        if !self.steps.is_empty() {
            self.wake.as_ref().ok_or("Start wake missing")?.wake()?;
        }
        step
    }
    fn run_key(&self) -> RunKey {
        self.run
    }
    fn launch_id(&self) -> LaunchId {
        self.launch
    }
    fn main_identity(&self) -> Option<crate::MainIdentity> {
        sugarsubstitute_contracts::execution::Generation::new(1).ok().map(|navigation| {
            crate::MainIdentity { run: self.run, launch: self.launch, navigation }
        })
    }
    fn main_construction_pending(&self) -> bool {
        false
    }
    fn observe(&mut self, observation: DesktopStartupObservation) -> Result<(), Box<dyn Error>> {
        match observation {
            DesktopStartupObservation::NativeRetired(run) if run == self.run => {
                self.calls.push("native_retired")
            }
            _ => return Err("Unexpected native fact".into()),
        }
        Ok(())
    }
    fn take_command(&mut self) -> Result<Option<DesktopStartupCommand>, Box<dyn Error>> {
        self.calls.push("command");
        Ok(None)
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
}
pub(super) fn budget() -> Result<NonZeroUsize, Box<dyn Error>> {
    Ok(NonZeroUsize::new(32).ok_or("Zero fixture budget")?)
}
