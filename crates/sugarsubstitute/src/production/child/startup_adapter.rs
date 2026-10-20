// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the Windows child owner to the shared startup controller.
use super::*;
use crate::startup::{BackendResult, PreparedStartup, StartupBackend};
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
    fn begin_shutdown(&mut self) -> BackendResult<ShutdownInitiation> {
        ProductionChild::begin_shutdown(self)
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        ProductionChild::shutdown_budget(self)
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
