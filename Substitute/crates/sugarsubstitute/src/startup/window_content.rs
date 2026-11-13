// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer the main window's startup contract: native observations, application commands,
//! canvas presentation and every close that keeps the user's work.
//!
//! Closes from the window, the operating system's session end and restart to apply all go
//! through [`StartupHost::request_close`]; the window reads close-save progress, the not-saved
//! notice and the configured language from the host.

use super::*;

impl<B: StartupBackend> StartupWindowContent for StartupHost<B> {
    #[cfg(all(windows, feature = "native-composition"))]
    fn main_composition_started(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<bool, Box<dyn Error>> {
        self.composition.as_mut().ok_or("Composition execution is unavailable")?.register(identity)
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn main_composition_wait(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        input: &mut Option<sugarsubstitute_desktop::canvas::CompositionWait>,
    ) -> Result<bool, Box<dyn Error>> {
        let now = self.execution.now()?;
        let cutoff = self.shutdown.budget().ok().map(|budget| budget.settle_until);
        let accepted = self
            .composition
            .as_mut()
            .ok_or("Composition execution is unavailable")?
            .offer(identity, input, now, cutoff)?;
        self.refresh_deadline()?;
        Ok(accepted)
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn main_canvas_presentation(
        &self,
    ) -> sugarsubstitute_desktop::canvas::MainCanvasPresentation<'_> {
        use sugarsubstitute_desktop::canvas::MainCanvasPresentation;
        let Some(canvas) = self.canvas.as_ref() else { return MainCanvasPresentation::Hide };
        let presentation = canvas.presentation();
        let granted = presentation.identity().and_then(|id| self.child.canvas_backend(id));
        if canvas.granted(granted) { presentation } else { MainCanvasPresentation::Hide }
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn main_canvas_input_region(
        &self,
    ) -> Option<sugarsubstitute_desktop::canvas::MainCanvasInputRegion<'_>> {
        let canvas = self.canvas.as_ref()?;
        let region = canvas.input_region()?;
        canvas.granted(self.child.canvas_backend(region.identity)).then_some(region)
    }

    #[cfg(windows)]
    fn main_canvas_animating(&self) -> bool {
        self.canvas.as_ref().is_some_and(|canvas| canvas.animating())
    }

    fn request_session_close(&mut self, critical: bool) -> Result<(), Box<dyn Error>> {
        self.request_close(
            sugarsubstitute_application::workspace_session::close_save::CloseReason::SessionEnding {
                critical,
            },
        )
    }

    /// Restart to apply a prepared update through the same close save as every other close,
    /// once the backend's running workflow finishes.
    fn request_restart_to_apply(&mut self) -> Result<(), Box<dyn Error>> {
        if self.workflow_restart.request(self.child.workflow_queue()) {
            self.restart_to_apply()
        } else {
            Ok(())
        }
    }

    fn update_status(&self) -> Option<sugarsubstitute_contracts::update::status::UpdateStatus> {
        let status = self.child.update_status()?.clone();
        Some(
            self.workflow_restart.presented(status, self.locale_messages().updates.restart_dropped),
        )
    }

    /// The coordinator's next one-time start-up update message, with the running version's
    /// release notes when this build names them.
    fn take_update_notice(&mut self) -> Option<sugarsubstitute_desktop::UpdateNotice> {
        use sugarsubstitute_contracts::update::status::StartupNotice;
        use sugarsubstitute_desktop::UpdateNotice;
        Some(match self.child.take_update_notice()? {
            StartupNotice::Updated(version) => {
                // Only a build with update configuration names its release notes.
                #[cfg(all(windows, feature = "windows-native-host"))]
                let changelog = crate::updates::release_notes(&version);
                #[cfg(not(all(windows, feature = "windows-native-host")))]
                let changelog = None;
                UpdateNotice::Updated { version, changelog }
            }
            StartupNotice::Repairing => UpdateNotice::Repairing,
            StartupNotice::PreviousVersionRunning => UpdateNotice::PreviousVersionRunning,
            StartupNotice::SugarCubes(line) => UpdateNotice::SugarCubes(line),
        })
    }

    /// Export the report the window shows, the latest failed attempt as presented, through the
    /// child's own export owner. The text is the report's plain text, as Copy uses.
    fn update_report_export(
        &mut self,
    ) -> Option<Box<dyn sugarsubstitute_desktop::report::surface::UpdateExport>> {
        let text = StartupWindowContent::update_status(self)?.failure()?.to_text();
        self.child.update_report_export(text)
    }

    fn workflow_activity(&self) -> sugarsubstitute_desktop::WorkflowActivity {
        match self.child.workflow_queue() {
            sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue::Busy => {
                sugarsubstitute_desktop::WorkflowActivity::Running
            }
            sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue::Idle
            | sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue::Unobserved => {
                sugarsubstitute_desktop::WorkflowActivity::Idle
            }
        }
    }

    fn close_save_settled(&self) -> bool {
        self.close_save_outcome().is_some()
    }

    fn session_end_reason(&self) -> &'static str {
        self.locale_messages().closing.saving
    }

    fn close_save_progress(
        &self,
    ) -> Option<sugarsubstitute_application::workspace_session::close_save::CloseSaveProgress> {
        Self::close_save_progress(self)
    }

    fn take_unsaved_notice(
        &mut self,
    ) -> Option<sugarsubstitute_application::workspace_session::close_save::NotSavedLastTime> {
        Self::take_unsaved_notice(self)
    }

    /// The configured language once startup has its session; English before then.
    fn locale_messages(&self) -> &'static sugarsubstitute_contracts::localization::Messages {
        let locale = self.startup.as_ref().map_or("en", |startup| {
            startup.session().configuration().presentation().locale().as_str()
        });
        sugarsubstitute_contracts::localization::messages(locale)
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn take_main_canvas_drag_out(
        &mut self,
    ) -> Option<sugarsubstitute_desktop::canvas::MainCanvasDragOut> {
        self.canvas.as_mut()?.take_drag_out()
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn main_canvas_presented(
        &mut self,
        target: sugarsubstitute_canvas::FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        if !matches!(
            self.main_canvas_presentation(),
            sugarsubstitute_desktop::canvas::MainCanvasPresentation::Frame(_)
        ) {
            return Ok(());
        }
        self.canvas.as_mut().ok_or("Application canvas disappeared")?.presented(target)
    }

    #[cfg(all(windows, feature = "native-composition"))]
    fn main_canvas_hidden(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(canvas) = &mut self.canvas {
            canvas.hidden(identity)?;
        }
        Ok(())
    }

    fn prepare_native_ui(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>> {
        self.consume(maximum)?;
        if self.closing {
            return Ok(StartupPreparationStep::Closing);
        }
        let step =
            self.child.prepare_native_ui(maximum.min(NonZeroUsize::MIN.saturating_add(31)))?;
        if step == StartupPreparationStep::Ready {
            self.adopt_prepared_startup()?;
            self.prepared = true;
        }
        if step == StartupPreparationStep::Closing {
            self.close_owned()?;
        }
        self.refresh_deadline()?;
        Ok(step)
    }
    fn run_key(&self) -> RunKey {
        self.policy.run()
    }
    fn launch_id(&self) -> sugarsubstitute_contracts::startup::LaunchId {
        self.startup.as_ref().map_or(self.policy.launch(), |startup| startup.session().launch())
    }
    fn main_identity(&self) -> Option<sugarsubstitute_contracts::startup::MainIdentity> {
        self.startup.as_ref().map(|startup| self.policy.identity(startup.session()))
    }
    fn main_construction_pending(&self) -> bool {
        self.child.main_construction_pending()
    }
    fn system_accent(&mut self, accent: Option<AccentColor>) -> Result<(), Box<dyn Error>> {
        self.adopt_system_accent(accent)
    }
    fn observe(&mut self, observation: DesktopStartupObservation) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "k1-native-qualification")]
        let observation = {
            let mut observation = Some(observation);
            if self.child.accept_preview_observation(&mut observation) {
                return self.refresh_deadline();
            }
            observation.ok_or("Ordinary native observation custody missing")?
        };
        let revealed = match &observation {
            DesktopStartupObservation::MainRevealed(identity) => Some(*identity),
            _ => None,
        };
        let phase = self.startup.as_ref().map(|startup| startup.session().phase());
        let session = self.startup.as_mut().map(PreparedStartup::session_mut);
        let route = self.policy.native(observation, session)?;
        self.route(route)?;
        self.report_page_failure();
        if let Some(identity) = revealed
            && let Some(startup) = &self.startup
            && !self.policy.closing()
            && startup.session().phase()
                == sugarsubstitute_application::startup::StartupPhase::Running
        {
            self.child.observe(DesktopStartupObservation::MainRevealed(identity), Some(startup))?;
            #[cfg(windows)]
            if let Some(canvas) = &mut self.canvas {
                let locale = startup.session().configuration().presentation().locale().as_str();
                let preference = startup.session().configuration().canvas_renderer();
                let policy = crate::native_canvas::gpu_policy::embedded_policy(preference)
                    .unwrap_or_else(|reason| {
                        tracing::warn!(
                            operation = "canvas_gpu_fallback",
                            reason,
                            "Stored canvas renderer preference is unusable; presenting CPU frames"
                        );
                        crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu
                    });
                canvas.start(self.execution.client()?, locale, policy)?;
            }
            #[cfg(feature = "k1-native-qualification")]
            self.child.preview_after_main_revealed(identity, startup.session());
        }
        self.invalidate(phase != self.startup.as_ref().map(|startup| startup.session().phase()));
        self.refresh_deadline()
    }
    fn take_command(&mut self) -> Result<Option<DesktopStartupCommand>, Box<dyn Error>> {
        if let Some(command) = self.commands.take() {
            return Ok(Some(command));
        }
        #[cfg(all(windows, any(feature = "windows-native-host", test)))]
        if let Some(command) = self.memory.as_mut().and_then(memory::MemoryGovernor::take_command) {
            return Ok(Some(command));
        }
        #[cfg(all(windows, any(feature = "windows-native-host", test)))]
        if let Some(command) = self.updates.take_command() {
            return Ok(Some(command));
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(command) = self.canvas.as_mut().and_then(canvas::EmbeddedCanvas::take_command) {
            return Ok(Some(command));
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(workspace) = &mut self.workspace
            && let Some(command) = workspace.next_command()?
        {
            return Ok(Some(command));
        }
        self.child.take_command()
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
            .into_iter()
            .chain(self.owned_deadlines())
            .chain(
                self.view
                    .as_ref()
                    .filter(|_| self.visible && !self.closing)
                    .and_then(StartupView::feedback_deadline),
            )
            .min()
    }
}

/// The splash line while this launch starts a new version for the first time: "Updating to
/// version X…" in place of the ordinary loading lines, until the coordinator confirms the
/// version; and "Updating SugarCubes…" while the host updates SugarCubes before ComfyUI loads
/// it. Failure, retry and cleanup lines always win, and a version label the splash cannot show
/// leaves the ordinary line.
pub(super) fn updating<B: StartupBackend>(
    child: &B,
    current: sugarsubstitute_splash::StartupStatus,
) -> Option<sugarsubstitute_splash::StartupStatus> {
    use sugarsubstitute_splash::StartupStatus;
    if !matches!(
        current,
        StartupStatus::Loading | StartupStatus::LoadingLong | StartupStatus::LoadingExtended
    ) {
        return None;
    }
    let status = child.update_status()?;
    if let Some(version) = status.applying() {
        return sugarsubstitute_splash::UpdateVersion::new(version).map(StartupStatus::Updating);
    }
    matches!(status.pack(), Some(sugarsubstitute_contracts::update::status::PackActivity::Updating))
        .then_some(StartupStatus::UpdatingSugarCubes)
}

impl<B: StartupBackend> StartupHost<B> {
    /// When this start's page readiness failed because of SugarCubes, tell the coordinator how,
    /// so the SugarCubes version just installed can be judged and restored. The child sends it
    /// once; a report that cannot go out is logged, and the start's own failure stands.
    fn report_page_failure(&mut self) {
        use sugarsubstitute_application::startup::SessionFailure;
        use sugarsubstitute_contracts::startup::ReadinessFailure;
        use sugarsubstitute_supervision::lifecycle_protocol::PageFailure;
        let failure = self.startup.as_ref().and_then(|startup| match startup.session().failure() {
            Some(SessionFailure::Frontend(probe)) => match probe.code {
                ReadinessFailure::SugarcubesUnavailable => Some(PageFailure::Unavailable),
                ReadinessFailure::SugarcubesFailed => Some(PageFailure::SetupFailed),
                ReadinessFailure::FrontendUnavailable | ReadinessFailure::TimedOut => None,
            },
            Some(SessionFailure::Adapter(_)) | None => None,
        });
        if let Some(failure) = failure
            && let Err(cause) = self.child.report_page_failure(failure)
        {
            tracing::warn!(operation = "sugarcubes_page_failure_report", cause = %cause,
                "The SugarCubes page failure could not be reported to the coordinator");
        }
    }
}
