// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the Windows child owner to the shared startup controller.
use super::*;
use crate::startup::{BackendResult, PreparedStartup, StartupBackend};
#[cfg(feature = "native-composition")]
use sugarsubstitute_application::startup::ConnectionBudgetKind;
impl StartupBackend for ProductionChild {
    type Prepared = PreparedChildStartup;
    fn start(
        &mut self,
        client: &ExecutionClient,
        scope: Scope,
        jobs: &JobSequence,
    ) -> BackendResult<()> {
        ProductionChild::start(self, client, scope, jobs)
    }
    fn consume(
        &mut self,
        batch: &CompletionBatch,
        prepared: Option<&Self::Prepared>,
    ) -> BackendResult<()> {
        ProductionChild::consume(self, batch, prepared)
    }
    fn prepare_native_ui(&mut self, max: NonZeroUsize) -> BackendResult<StartupPreparationStep> {
        ProductionChild::prepare_native_ui(self, max)
    }
    fn take_prepared_startup(&mut self) -> Option<Self::Prepared> {
        ProductionChild::take_prepared_startup(self)
    }
    fn apply(&mut self, effect: StartupEffect, prepared: &mut Self::Prepared) -> BackendResult<()> {
        ProductionChild::apply(self, effect, prepared)
    }
    fn observe(
        &mut self,
        observation: DesktopStartupObservation,
        prepared: Option<&Self::Prepared>,
    ) -> BackendResult<()> {
        ProductionChild::observe(self, observation, prepared)
    }
    fn system_accent(
        &mut self,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    ) -> BackendResult<()> {
        self.adopt_system_accent(accent)
    }
    #[cfg(feature = "native-composition")]
    fn canvas_admission(
        &self,
        identity: MainIdentity,
        lease: &MemoryLease,
    ) -> std::result::Result<(), crate::startup::CanvasRefusal> {
        self.admission(identity, lease)
    }
    #[cfg(feature = "native-composition")]
    fn canvas_backend(&self, identity: MainIdentity) -> Option<crate::startup::CanvasBackend> {
        let backend = self.canvas_backend?;
        if self.shutdown.closing()
            || self.attempt.is_some()
            || self.identity != identity
            || backend.identity != identity
            || self.phase != Some(sugarsubstitute_application::startup::StartupPhase::Running)
            || self.lease != Some(backend.lease)
        {
            return None;
        }
        let current = match self.connection_budget?.kind() {
            ConnectionBudgetKind::Attached => self.running_backend.is_none(),
            ConnectionBudgetKind::Managed => {
                self.running_backend
                    .as_ref()
                    .and_then(|running| running.reload_lease(identity).ok())
                    == Some(backend.lease)
            }
        };
        current.then_some(backend)
    }
    fn workspace_context(
        &self,
        prepared: &Self::Prepared,
    ) -> BackendResult<Option<crate::startup::WorkspaceContext>> {
        let Some(backend) = self.backend_binding else {
            return Ok(None);
        };
        if self.shutdown.closing()
            || self.lease.is_none()
            || self.attempt.is_some()
            || self
                .running_backend
                .as_ref()
                .is_some_and(|running| running.reload_lease(self.identity).is_err())
        {
            return Ok(None);
        }
        let installation =
            self.session_installation.clone().ok_or("Authenticated host correlation absent")?;
        let origin =
            prepared.session().connection_origin().ok_or("Authenticated frontend origin absent")?;
        Ok(Some(crate::startup::WorkspaceContext {
            identity: self.identity,
            profile: prepared.session().configuration().profile().clone(),
            installation,
            backend,
            origin,
            peer: self.session_peer.ok_or("Authenticated listener peer absent")?,
        }))
    }
    fn configure_workspace(
        &mut self,
        bootstrap: sugarsubstitute_desktop::SessionBootstrap,
        placement: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    ) -> BackendResult<()> {
        if self.main.pending() || self.main.request_consumed {
            return Err("Native main construction already admitted".into());
        }
        self.workspace_bootstrap = Some(bootstrap);
        self.workspace_placement = placement;
        Ok(())
    }
    fn main_construction_pending(&self) -> bool {
        ProductionChild::main_construction_pending(self)
    }
    fn console_text(&self) -> Option<&dyn sugarsubstitute_desktop::ConsoleText> {
        ProductionChild::console_text(self)
    }
    fn transcript(&self) -> Option<OutputSnapshot> {
        ProductionChild::transcript(self)
    }
    fn take_startup_event(&mut self) -> BackendResult<Option<StartupEvent>> {
        ProductionChild::take_startup_event(self)
    }
    fn take_command(&mut self) -> BackendResult<Option<DesktopStartupCommand>> {
        ProductionChild::take_command(self)
    }
    fn next_deadline(&self) -> Option<Moment> {
        ProductionChild::next_deadline(self)
    }
    fn cleanup_failed(&self) -> bool {
        ProductionChild::cleanup_failed(self)
    }
    fn ensure_retry_ready(&self, identity: MainIdentity) -> BackendResult<()> {
        ProductionChild::ensure_retry_ready(self, identity)
    }
    fn retry_attempt(
        &mut self,
        identity: MainIdentity,
        prepared: &mut Self::Prepared,
    ) -> BackendResult<()> {
        ProductionChild::retry_attempt(self, identity, prepared)
    }
    fn advance_reload(&mut self, prepared: &mut Self::Prepared) -> BackendResult<()> {
        ProductionChild::advance_reload(self, prepared)
    }
    fn begin_workspace_close(&mut self) -> BackendResult<ShutdownInitiation> {
        ProductionChild::begin_workspace_close(self)
    }
    fn begin_shutdown(&mut self) -> BackendResult<ShutdownInitiation> {
        ProductionChild::begin_shutdown(self)
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        ProductionChild::shutdown_budget(self)
    }
    fn update_status(&self) -> Option<&sugarsubstitute_contracts::update::status::UpdateStatus> {
        self.bootstrap.coordinator_updates().status()
    }
    fn workflow_queue(&self) -> sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue {
        self.bootstrap.coordinator_updates().workflow()
    }
    fn take_update_notice(
        &mut self,
    ) -> Option<sugarsubstitute_contracts::update::status::StartupNotice> {
        self.bootstrap.channel_mut().updates_mut().take_notice()
    }
    fn report_page_failure(
        &mut self,
        failure: sugarsubstitute_supervision::lifecycle_protocol::PageFailure,
    ) -> BackendResult<()> {
        ProductionChild::report_page_failure(self, failure)
    }
    fn update_report_export(
        &mut self,
        text: String,
    ) -> Option<Box<dyn sugarsubstitute_desktop::report::surface::UpdateExport>> {
        match super::update_export::UpdateReportExport::new(
            text,
            sugarsubstitute_execution::MonotonicClock::new(),
        ) {
            Ok(export) => Some(Box::new(export)),
            Err(cause) => {
                tracing::error!(operation = "update_report_export", cause = %cause,
                    "The update report cannot be exported; Copy stays available");
                None
            }
        }
    }
    fn request_restart_to_apply(&mut self) -> BackendResult<()> {
        ProductionChild::request_restart_to_apply(self)
    }
    fn request_shutdown(&mut self, budget: ShutdownBudget) -> BackendResult<()> {
        ProductionChild::request_shutdown(self, budget)
    }
    fn poll_shutdown(&mut self) -> BackendResult<bool> {
        ProductionChild::poll_shutdown(self)
    }
    fn finish_shutdown(&mut self) -> BackendResult<()> {
        ProductionChild::finish_shutdown(self)
    }
    fn startup_cause_code(
        cause: &(dyn Error + 'static),
    ) -> sugarsubstitute_desktop::StartupCauseCode {
        ProductionChild::startup_cause_code(cause)
    }
    fn close_diagnostic(
        &self,
        cause: &(dyn Error + 'static),
    ) -> (sugarsubstitute_desktop::StartupCaptureState, sugarsubstitute_desktop::StartupCauseCode)
    {
        ProductionChild::close_diagnostic(self, cause)
    }
    #[cfg(feature = "k1-native-qualification")]
    fn preview_after_main_revealed(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        session: &sugarsubstitute_application::startup::StartupSession,
    ) {
        ProductionChild::preview_after_main_revealed(self, identity, session)
    }
    #[cfg(feature = "k1-native-qualification")]
    fn accept_preview_observation(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> bool {
        ProductionChild::accept_preview_observation(self, observation)
    }
}
impl PreparedStartup for PreparedChildStartup {
    fn session(&self) -> &sugarsubstitute_application::startup::StartupSession {
        self.session()
    }
    fn session_mut(&mut self) -> &mut sugarsubstitute_application::startup::StartupSession {
        self.session_mut()
    }
    fn release_source(&mut self) {
        self.release_source();
    }
}

#[cfg(all(test, feature = "native-composition"))]
#[path = "startup_adapter/tests.rs"]
mod tests;
