// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the real captured child, authenticated lifecycle channel and native startup effects.
#[path = "child/acquisition_retirement.rs"]
mod acquisition_retirement;
#[path = "child/attempt.rs"]
mod attempt;
#[path = "child/backend.rs"]
mod backend;
#[path = "child/bootstrap.rs"]
mod bootstrap;
#[path = "child/capture.rs"]
mod capture;
#[path = "child/channel.rs"]
mod channel;
#[path = "child/configuration.rs"]
mod configuration;
#[path = "child/connection.rs"]
mod connection;
#[path = "child/diagnostics.rs"]
mod diagnostics;
#[path = "child/handshake.rs"]
mod handshake;
#[path = "child/main.rs"]
mod main;
#[path = "child/output.rs"]
mod output;
#[path = "child/panel.rs"]
mod panel;
#[cfg(feature = "k1-native-qualification")]
#[path = "child/preview.rs"]
mod preview;
#[path = "child/profile_transaction.rs"]
mod profile_transaction;
#[cfg(feature = "k1-native-qualification")]
#[path = "child/qualification.rs"]
mod qualification;
#[path = "child/readiness.rs"]
mod readiness;
#[path = "child/reload.rs"]
mod reload;
#[path = "child/requests.rs"]
mod requests;
#[path = "child/retirement.rs"]
mod retirement;
#[path = "child/startup_adapter.rs"]
mod startup_adapter;
use crate::startup::ShutdownInitiation;
#[cfg(test)]
#[path = "child/test_support.rs"]
pub(crate) mod test_support;
use crate::{
    job_sequence::JobSequence,
    production_output::{OutputOwner, OutputSnapshot},
};
use bootstrap::ChildBootstrap;
pub(crate) use configuration::PreparedChildStartup;
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_application::startup::ConnectionBudget;
use sugarsubstitute_contracts::{execution::*, lifecycle::*, startup::*};
use sugarsubstitute_desktop::{
    DesktopStartupCommand, DesktopStartupObservation, MainWindowSettings, StartupChildFailure,
    StartupChildStep as Step, StartupPreparationStep,
};
use sugarsubstitute_execution::{
    ExecutionClient, ProcessLifetimeCapture,
    windows_lifecycle_pipe::{
        InheritedLifecycleBootstrap, LifecyclePipeDeadline, ReceivedLifecycleFrame,
    },
};
use sugarsubstitute_supervision::lifecycle_protocol::{LeaseReference, WireHeader};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Name the consumer step whose call returned an error, keeping an inner step already named.
fn step<T>(step: Step, result: Result<T>) -> Result<T> {
    result.map_err(|cause| StartupChildFailure::retain(step, cause))
}
/// Inert application role data; paths/settings grant no native profile or backend privilege.
pub(in crate::production) struct ChildInputs {
    pub(in crate::production) qualification: super::qualification::QualificationIntent,
    pub(in crate::production) diagnostics: crate::diagnostics::DiagnosticHealth,
    pub(in crate::production) input: InheritedLifecycleBootstrap,
    pub(in crate::production) original: LifecyclePipeDeadline,
    pub(in crate::production) run: RunKey,
    pub(in crate::production) native_scope: ScopeSpec,
    pub(in crate::production) identity: (LaunchId, Generation),
    pub(in crate::production) settings: MainWindowSettings,
}
/// Keep all partial admissions, original frames, completions and permanent capture together.
/// This owner creates no executor and uses only the application's shared job sequence.
pub(crate) struct ProductionChild {
    readiness: Option<readiness::Readiness>,
    diagnostics: crate::diagnostics::DiagnosticHealth,
    bootstrap: ChildBootstrap,
    run: RunKey,
    identity: MainIdentity,
    client: Option<ExecutionClient>,
    jobs: Option<JobSequence>,
    connection_budget: Option<ConnectionBudget>,
    acquisition_header: Option<WireHeader>,
    attempt: Option<attempt::AttemptCleanup>,
    reload: Option<reload::FrontendReload>,
    acquire: Option<connection::PendingAcquire>,
    lease: Option<LeaseReference>,
    grant: Option<ReceivedLifecycleFrame>,
    grant_header: Option<WireHeader>,
    main: main::MainPreparation,
    output: output::ChildOutput,
    panel: Option<panel::PanelOwner>,
    panel_deliveries: panel::Deliveries,
    panel_owner_admission_warned: bool,
    /// Latest operating-system accent, presented to every new SugarCubes page.
    system_accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    running_backend: Option<backend::RunningBackendBinding>,
    /// Parent-authenticated canonical selection correlation; native grants remain independent.
    backend_binding: Option<BackendBindingDigest>,
    session_peer: Option<sugarsubstitute_supervision::lifecycle_protocol::BackendPeer>,
    session_installation: Option<sugarsubstitute_contracts::installation::InstallationId>,
    workspace_bootstrap: Option<sugarsubstitute_desktop::SessionBootstrap>,
    workspace_placement: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    #[cfg(feature = "native-composition")]
    canvas_backend: Option<crate::startup::CanvasBackend>,
    snapshot: Option<OutputSnapshot>,
    commands: [Option<DesktopStartupCommand>; 4],
    events: [Option<StartupEvent>; 2],
    observation: Option<DesktopStartupObservation>,
    failed_frame: Option<ReceivedLifecycleFrame>,
    failure: Option<Box<dyn Error>>,
    failed: bool,
    shutdown: retirement::ChildShutdown,
    permanent: Option<ProcessLifetimeCapture>,
    prepared: bool,
    phase: Option<sugarsubstitute_application::startup::StartupPhase>,
    probe_queued: bool,
    finished: bool,
}
impl ProductionChild {
    /// Construct the real once-only native driver factory without invoking native effects.
    /// # Errors
    /// Preserves driver-factory or original native-scope/identity rejection before admission.
    pub(in crate::production) fn new(inputs: ChildInputs) -> Result<Self> {
        if inputs.native_scope.deadline.is_some() {
            return Err("Persistent native scope has a finite deadline".into());
        }
        let driver = sugarsubstitute_crash::desktop_capture_driver()?;
        let bootstrap = ChildBootstrap::new(
            channel::ChildChannel::new(inputs.input, inputs.original, inputs.run),
            capture::ChildCapture::new(driver),
            inputs.native_scope,
            inputs.identity,
        )?
        .with_qualification(inputs.qualification, inputs.run);
        Ok(Self::assemble(
            bootstrap,
            inputs.run,
            inputs.identity,
            inputs.settings,
            inputs.diagnostics,
        ))
    }
    /// Assemble inert application state around its actual bootstrap owner.
    /// Native driver selection remains exclusively at the outer constructor boundary.
    fn assemble(
        bootstrap: ChildBootstrap,
        run: RunKey,
        identity: (LaunchId, Generation),
        settings: MainWindowSettings,
        diagnostics: crate::diagnostics::DiagnosticHealth,
    ) -> Self {
        Self {
            diagnostics,
            bootstrap,
            run,
            identity: MainIdentity { run, launch: identity.0, navigation: identity.1 },
            client: None,
            jobs: None,
            connection_budget: None,
            acquisition_header: None,
            attempt: None,
            reload: None,
            acquire: None,
            lease: None,
            grant: None,
            grant_header: None,
            main: main::MainPreparation::new(settings),
            output: output::ChildOutput::new(),
            panel: None,
            panel_deliveries: panel::Deliveries::new(),
            panel_owner_admission_warned: false,
            system_accent: None,
            running_backend: None,
            backend_binding: None,
            session_peer: None,
            session_installation: None,
            workspace_bootstrap: None,
            workspace_placement: None,
            #[cfg(feature = "native-composition")]
            canvas_backend: None,
            snapshot: None,
            commands: std::array::from_fn(|_| None),
            events: std::array::from_fn(|_| None),
            observation: None,
            failed_frame: None,
            failure: None,
            failed: false,
            shutdown: retirement::ChildShutdown::new(),
            permanent: None,
            prepared: false,
            phase: None,
            readiness: None,
            probe_queued: false,
            finished: false,
        }
    }
    /// Transfer the genuine child scope before any fallible bootstrap admission.
    /// # Errors
    /// Retain the original client/scope/job identities through partial failure cleanup.
    pub(crate) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: Scope,
        jobs: &JobSequence,
    ) -> Result<()> {
        let result: Result<_> = (|| -> Result<_> {
            if self.client.is_some() {
                return Err("Child cannot start twice".into());
            }
            self.client = Some(client.clone());
            self.jobs = Some(jobs.clone());
            self.bootstrap.start(client, scope, jobs)?;
            Ok(())
        })();
        self.failed |= result.is_err();
        self.bootstrap.report_preparation();
        let result = self.observe_diagnostics(result);
        self.failed |= result.is_err();
        result
    }
    /// Deliver one immutable batch to every actual owner before resolving any failure.
    /// # Errors
    /// Retain matching typed terminals and every native frame on failure.
    pub(crate) fn consume(
        &mut self,
        batch: &CompletionBatch,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let result: Result<_> = (|| -> Result<_> {
            if let Some(prepared) = prepared {
                self.phase = Some(prepared.session().phase());
            }
            self.probe_queued = false;
            let bootstrap = self.bootstrap.consume(batch);
            self.main.capture(batch);
            self.output.capture(batch);
            if let Err(cause) = bootstrap {
                return Err(StartupChildFailure::retain(Step::Bootstrap, Box::new(cause)));
            }
            if step(Step::AcquisitionExpiry, self.expire_acquisition())? {
                return Ok(());
            }
            step(Step::Main, self.consume_main())?;
            step(Step::Output, self.consume_output())?;
            self.activity(prepared)?;
            step(Step::Panel, self.advance_panel())
        })();
        self.failed |= result.is_err();
        self.bootstrap.report_preparation();
        if !self.shutdown.closing() && self.diagnostics.check().is_err() {
            self.failed = true;
            return step(Step::Diagnostics, self.observe_diagnostics(result));
        }
        match result {
            Err(cause) if self.attempt.is_some() && !self.shutdown.closing() => {
                step(Step::AttemptRetention, self.retain_attempt_failure(cause))?;
                Ok(())
            }
            result => result,
        }
    }
    /// Drive only actual capture/authentication/configuration before native surface admission.
    /// # Errors
    /// No positive preparation result exists until the same actual bootstrap returns Ready.
    pub(crate) fn prepare_native_ui(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<StartupPreparationStep> {
        let result: Result<_> = (|| -> Result<_> {
            let step = self.bootstrap.prepare_native_ui(max)?;
            if step == StartupPreparationStep::Ready {
                self.prepared = true;
            }
            Ok(step)
        })();
        self.failed |= result.is_err();
        self.bootstrap.report_preparation();
        let result = self.observe_diagnostics(result);
        self.failed |= result.is_err();
        result
    }
    /// Move the complete guarded application state once after actual Ready.
    pub(crate) fn take_prepared_startup(&mut self) -> Option<PreparedChildStartup> {
        if self.prepared { self.bootstrap.take_prepared_startup() } else { None }
    }
    /// Execute the sole application's decision with borrowed actual configuration custody.
    /// # Errors
    /// Reject wrong attempts/phases or duplicate work before further admission.
    pub(crate) fn apply(
        &mut self,
        effect: StartupEffect,
        prepared: &mut PreparedChildStartup,
    ) -> Result<()> {
        let result: Result<_> = (|| -> Result<_> {
            if self.shutdown.closing() {
                return match effect {
                    StartupEffect::CleanupConnection { .. } => Ok(()),
                    _ => Err("Child effect admission closed".into()),
                };
            }
            if prepared.session().launch() != self.identity.launch || prepared.source().is_none() {
                return Err("Actual startup source/attempt missing".into());
            }
            match effect {
                StartupEffect::PrepareConnection { launch } if launch == self.identity.launch => {
                    self.begin_connection(prepared)
                }
                StartupEffect::LoadFrontend { launch, navigation, origin }
                    if launch == self.identity.launch && navigation == self.identity.navigation =>
                {
                    self.begin_main(prepared, origin)
                }
                StartupEffect::CleanupConnection { launch, .. }
                    if launch == self.identity.launch =>
                {
                    let sugarcubes_failed = prepared.session().failure().is_some_and(
                        sugarsubstitute_application::startup::SessionFailure::sugarcubes_failed,
                    );
                    self.begin_attempt_cleanup(sugarcubes_failed)?;
                    Ok(())
                }
                _ => Err("Effect belongs to native presentation or another attempt".into()),
            }
        })();
        self.failed |= result.is_err();
        result
    }
    /// Preserve the actual callback before profile/IPC handling; no inert key grants authority.
    /// # Errors
    /// Failure retains the unchanged observation and first underlying cause.
    pub(crate) fn observe(
        &mut self,
        observation: DesktopStartupObservation,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let result: Result<_> = (|| -> Result<_> {
            if self.observation.is_some() {
                return Err("Original native observation remains unconsumed".into());
            }
            self.observation = Some(observation);
            self.observe_native(prepared)
        })();
        self.failed |= result.is_err();
        result
    }
    /// Borrow actual retained Main construction/input/jobs for native absence checks.
    pub(crate) fn main_construction_pending(&self) -> bool {
        self.main.construction_pending()
    }
    /// Borrow the actual immutable native-console source while its complete original
    /// source custody remains with this child. The affine adapter copies before this borrow ends.
    pub(crate) fn console_text(&self) -> Option<&dyn sugarsubstitute_desktop::ConsoleText> {
        self.snapshot.as_ref().map(|view| view as &dyn sugarsubstitute_desktop::ConsoleText)
    }
    /// Borrow only the actual bounded source-custodied transcript view.
    pub(crate) fn transcript(&self) -> Option<OutputSnapshot> {
        self.snapshot.clone()
    }
    /// Transfer one original application observation after actual semantic consumption.
    pub(crate) fn take_startup_event(&mut self) -> Result<Option<StartupEvent>> {
        Ok(take_first(&mut self.events))
    }
    /// Transfer one native command; real challenge commands retain channel provenance.
    /// # Errors
    /// Preserves actual challenge clock/admission failure.
    pub(crate) fn take_command(&mut self) -> Result<Option<DesktopStartupCommand>> {
        let result: Result<_> = (|| -> Result<_> {
            if let Some(command) = take_first(&mut self.commands) {
                return Ok(Some(command));
            }
            if !self.shutdown.closing()
                && let Some(command) = self.panel.as_mut().and_then(panel::PanelOwner::take_command)
            {
                return Ok(Some(DesktopStartupCommand::SendPanel(command)));
            }
            if let Some(reply) = self.panel_deliveries.take_command(self.identity) {
                return Ok(Some(reply));
            }
            if !self.shutdown.closing()
                && !self.probe_queued
                && self.phase
                    == Some(sugarsubstitute_application::startup::StartupPhase::WebviewLoading)
                && self
                    .main
                    .profile()
                    .is_some_and(profile_transaction::ProfileTransaction::authorized)
            {
                self.probe_queued = true;
                return Ok(Some(DesktopStartupCommand::ProbeFrontend(self.identity)));
            }
            self.bootstrap.channel_mut().take_ui_command()
        })();
        self.failed |= result.is_err();
        result
    }
    /// Borrow the earliest original pending operation or cleanup cutoff without renewing it.
    pub(crate) fn next_deadline(&self) -> Option<Moment> {
        if self.cleanup_failed() && !self.shutdown.closing() {
            return self.bootstrap.ui_deadline();
        }
        [
            (!self.prepared && !self.shutdown.closing())
                .then(|| self.bootstrap.original_deadline())
                .flatten(),
            self.bootstrap.ui_deadline(),
            #[cfg(feature = "k1-native-qualification")]
            self.bootstrap.preview_deadline(),
            self.acquire.filter(|p| !p.retiring).and_then(connection::PendingAcquire::deadline),
            self.main.deadline().filter(|_| self.attempt.is_none()),
            self.output.deadline().filter(|_| self.attempt.is_none()),
            self.running_backend
                .as_ref()
                .and_then(backend::RunningBackendBinding::retry_deadline)
                .filter(|_| !self.shutdown.closing() && !self.output.replacement_stopped()),
            self.shutdown.deadline(),
            self.readiness
                .as_ref()
                .filter(|_| !self.shutdown.closing())
                .and_then(readiness::Readiness::deadline),
            self.reload
                .as_ref()
                .filter(|_| !self.shutdown.closing())
                .and_then(|reload| reload.deadline()),
            self.attempt.as_ref().and_then(attempt::AttemptCleanup::deadline),
            self.panel
                .as_ref()
                .filter(|_| !self.shutdown.closing())
                .and_then(panel::PanelOwner::deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    /// Use only the actual session accepted by StartupPolicy after current MainRevealed.
    #[cfg(feature = "k1-native-qualification")]
    pub(crate) fn preview_after_main_revealed(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        session: &sugarsubstitute_application::startup::StartupSession,
    ) {
        if let Some(command) = self.bootstrap.preview_after_main_revealed(identity, session)
            && self.push_command(command).is_err()
        {
            self.bootstrap.reject_preview_command();
        }
    }
    /// Consume only qualification payloads before they reach ordinary startup failure policy.
    #[cfg(feature = "k1-native-qualification")]
    pub(crate) fn accept_preview_observation(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> bool {
        self.bootstrap.accept_preview_observation(observation)
    }
    fn client(&self) -> Result<&ExecutionClient> {
        self.client.as_ref().ok_or_else(|| "Actual child client absent".into())
    }
    fn next_job(&self) -> Result<JobId> {
        Ok(self.jobs.as_ref().ok_or("Actual shared job sequence absent")?.allocate()?)
    }
    fn push_event(&mut self, event: StartupEvent) -> Result<()> {
        let slot = self
            .events
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("Bounded startup event slots full")?;
        *slot = Some(event);
        Ok(())
    }
    fn push_command(&mut self, command: DesktopStartupCommand) -> Result<()> {
        #[cfg(feature = "k1-native-qualification")]
        let retire = match &command {
            DesktopStartupCommand::RetireMain { identity, .. } => Some(*identity),
            _ => None,
        };
        let slot = self
            .commands
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("Bounded desktop command slots full")?;
        *slot = Some(command);
        #[cfg(feature = "k1-native-qualification")]
        if let Some(identity) = retire {
            self.bootstrap.retire_preview_main(identity);
        }
        Ok(())
    }
}
fn take_first<T, const N: usize>(slots: &mut [Option<T>; N]) -> Option<T> {
    let value = slots[0].take();
    for i in 1..N {
        slots[i - 1] = slots[i].take();
    }
    value
}

#[cfg(test)]
#[path = "child/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "child/frontend_lifetime_tests.rs"]
mod frontend_lifetime_tests;
