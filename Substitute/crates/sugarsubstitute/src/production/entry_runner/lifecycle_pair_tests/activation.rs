// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The activation adapter of the coordinator peer: one desktop's real actor until its process
//! exits, then what the production activation keeps of it.
use super::{clock::*, *};
use crate::{
    lifecycle_harness::WireEnd,
    production::{coordinator_backend::RetainedBackend, parent_lifecycle::DesktopPeer},
};
use sugarsubstitute_contracts::execution::Clock;

/// What the production activation keeps of a desktop after its process exited.
#[derive(Clone, Copy, Debug)]
pub(super) struct Retired {
    normal: bool,
    restart_to_apply: bool,
    budget: Option<ShutdownBudget>,
}

/// The activation adapter: one desktop's real actor, then its retirement record.
pub(super) struct WireActivation {
    pub(super) wire: WireEnd,
    pub(super) clock: Arc<VirtualClock>,
    pub(super) desktop: Option<DesktopPeer>,
    pub(super) retired: Option<Retired>,
    pub(super) backend: Option<RetainedBackend>,
    pub(super) failure: Option<Box<dyn Error>>,
    pub(super) budget: Option<ShutdownBudget>,
}
impl WireActivation {
    fn ask_to_close(&mut self) {
        let now = self.clock.now();
        if let (Some(desktop), Some(budget)) = (&mut self.desktop, self.budget)
            && desktop.ask_to_close(now, budget)
        {
            self.wire.log().note("coordinator: the live window is asked to close");
        }
    }
    pub(super) fn retire(&mut self, how: &str) -> HarnessResult {
        let Some(mut desktop) = self.desktop.take() else { return Ok(()) };
        let snapshot = desktop.exit_snapshot(self.clock.now())?;
        let normal = desktop.completed()
            && matches!(
                snapshot.exit,
                Some(
                    sugarsubstitute_contracts::lifecycle::DesktopExitKind::Normal
                        | sugarsubstitute_contracts::lifecycle::DesktopExitKind::StartupCancelled
                )
            );
        let retired = Retired {
            normal,
            restart_to_apply: normal && desktop.parent().restart_to_apply_requested(),
            budget: desktop.parent().shutdown(),
        };
        self.wire.log().note(format!(
            "coordinator: the desktop is gone ({how}): exit {:?}, normal close {}, restart to apply {}",
            snapshot.exit, retired.normal, retired.restart_to_apply
        ));
        self.retired = Some(retired);
        Ok(())
    }
}
impl EntryActivation for WireActivation {
    fn presentation_locale(&self) -> Option<sugarsubstitute_contracts::configuration::LocaleId> {
        None
    }
    fn enter_activation(
        &mut self,
        _: &mut Option<AuthenticatedEntryLaunch>,
        _: EntryResources<'_>,
    ) -> Result<(), Box<dyn Error>> {
        Err("The pair's desktop is already admitted".into())
    }
    fn consume(
        &mut self,
        _: &CompletionBatch,
        _: EntryResources<'_>,
    ) -> Result<HeadlessStep, Box<dyn Error>> {
        Ok(HeadlessStep::Active)
    }
    fn request_close(&mut self, budget: Option<ShutdownBudget>) -> Result<(), Box<dyn Error>> {
        if let Some(budget) = budget {
            self.budget = Some(self.budget.map_or(budget, |old| ShutdownBudget {
                started: old.started.min(budget.started),
                graceful_until: old.graceful_until.min(budget.graceful_until),
                settle_until: old.settle_until.min(budget.settle_until),
            }));
        }
        self.ask_to_close();
        Ok(())
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.desktop
            .as_ref()
            .and_then(|desktop| desktop.parent().shutdown())
            .or(self.retired.and_then(|retired| retired.budget))
    }
    fn normal_close_requested(&self) -> bool {
        self.retired.is_some_and(|retired| retired.normal)
    }
    fn publish_update_status(
        &mut self,
        revision: u64,
        status: &sugarsubstitute_contracts::update::status::UpdateStatus,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(desktop) = &mut self.desktop {
            desktop
                .parent_mut()
                .publish_update_status(revision, status)
                .map_err(|cause| cause.to_string())?;
        }
        Ok(())
    }
    fn sugarcubes_update(&self) -> sugarsubstitute_contracts::update::pack::PackUpdateStatus {
        self.backend
            .as_ref()
            .map(|retained| retained.backend.pack_update_status())
            .unwrap_or_default()
    }
    fn hold_sugarcubes_update(&mut self, held: bool) {
        if let Some(retained) = &mut self.backend {
            retained.backend.hold_pack_update(held);
        }
    }
    fn sugarcubes_installing(&self) -> bool {
        self.backend.as_ref().is_some_and(|retained| retained.backend.pack_installing())
    }
    fn restart_to_apply_requested(&self) -> bool {
        self.desktop.as_ref().is_some_and(|desktop| desktop.parent().restart_to_apply_requested())
            || self.retired.is_some_and(|retired| retired.restart_to_apply)
    }
    fn take_update_check_request(&mut self) -> bool {
        self.desktop.as_mut().is_some_and(|desktop| desktop.parent_mut().take_check_request())
    }
    fn has_desktop(&self) -> bool {
        self.desktop.is_some()
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        self.failure.as_deref()
    }
    fn failed_process(&self) -> sugarsubstitute_contracts::installation::FailedProcess {
        sugarsubstitute_contracts::installation::FailedProcess::Host
    }
    fn desktop_close_intent(&self) -> Option<CompletionIntent> {
        if self.retired.is_some_and(|retired| retired.normal) {
            return Some(CompletionIntent::Normal);
        }
        self.desktop.as_ref()?.parent().close_progress_request().map(|request| request.intent())
    }
    fn next_deadline(&self) -> Option<Instant> {
        let due = self.desktop.as_ref()?.deadlines().into_iter().filter_map(elapsed_of).min()?;
        Some(sugarsubstitute_execution::qualification_time::anchor() + BASE + due)
    }
    fn poll_close(
        &mut self,
        _: &CompletionBatch,
        _: Option<EntryResources<'_>>,
        budget: ShutdownBudget,
    ) -> Result<bool, Box<dyn Error>> {
        self.request_close(Some(budget))?;
        Ok(self.desktop.is_none()
            && self.backend.as_ref().is_none_or(|retained| retained.backend.settled()))
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
