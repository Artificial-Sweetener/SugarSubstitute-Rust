// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise shared startup ordering with an adapter that contains no OS handles or APIs.
use super::*;
use sugarsubstitute_application::startup::{StartupPhase, StartupSession};
use sugarsubstitute_contracts::{configuration::*, execution::*, lifecycle::*, startup::*};
use sugarsubstitute_desktop::{StartupCaptureState, StartupCauseCode};
use sugarsubstitute_execution::{ExecutionClient, MonotonicClock};

pub(super) struct Prepared {
    pub(super) session: StartupSession,
    pub(super) released: bool,
}
impl PreparedStartup for Prepared {
    fn session(&self) -> &StartupSession {
        &self.session
    }
    fn session_mut(&mut self) -> &mut StartupSession {
        &mut self.session
    }
    fn release_source(&mut self) {
        self.released = true;
    }
}
#[derive(Default)]
pub(super) struct Platform {
    pub(in crate::startup) budget: Option<ShutdownBudget>,
    pub(in crate::startup) shutdown_requests: Vec<ShutdownBudget>,
    pub(in crate::startup) restart_requests: usize,
    pub(in crate::startup) shutdown_starts: usize,
    retired: bool,
    finished: bool,
    effects: Vec<&'static str>,
    revealed: Vec<MainIdentity>,
    accents: Vec<Option<sugarsubstitute_contracts::appearance::AccentColor>>,
    pub(in crate::startup) start_allowed: bool,
    reload_failure: Option<Box<dyn Error>>,
    /// The coordinator status this fake child last received.
    pub(in crate::startup) update_status:
        Option<sugarsubstitute_contracts::update::status::UpdateStatus>,
    /// Start-up messages this fake child still has for the window.
    pub(in crate::startup) notices:
        std::collections::VecDeque<sugarsubstitute_contracts::update::status::StartupNotice>,
    /// Report texts the window asked this fake child to export.
    pub(in crate::startup) exported: Vec<String>,
    /// SugarCubes page failures this fake child was asked to report.
    pub(in crate::startup) page_failures:
        Vec<sugarsubstitute_supervision::lifecycle_protocol::PageFailure>,
}
impl StartupBackend for Platform {
    type Prepared = Prepared;
    fn start(&mut self, _: &ExecutionClient, _: Scope, _: &JobSequence) -> BackendResult<()> {
        if self.start_allowed { Ok(()) } else { Err("unused native startup boundary".into()) }
    }
    fn consume(&mut self, _: &CompletionBatch, _: Option<&Prepared>) -> BackendResult<()> {
        Ok(())
    }
    fn prepare_native_ui(&mut self, _: NonZeroUsize) -> BackendResult<StartupPreparationStep> {
        Err("unused native preparation boundary".into())
    }
    fn take_prepared_startup(&mut self) -> Option<Prepared> {
        None
    }
    fn apply(&mut self, effect: StartupEffect, _: &mut Prepared) -> BackendResult<()> {
        self.effects.push(match effect {
            StartupEffect::PrepareConnection { .. } => "prepare",
            StartupEffect::CleanupConnection { .. } => "cleanup",
            _ => return Err("unexpected native effect".into()),
        });
        Ok(())
    }
    fn observe(
        &mut self,
        observation: DesktopStartupObservation,
        prepared: Option<&Prepared>,
    ) -> BackendResult<()> {
        if let DesktopStartupObservation::MainRevealed(identity) = observation {
            if prepared.is_none_or(|value| value.session.phase() != StartupPhase::Running) {
                return Err(
                    "reveal reached backend before shared startup policy accepted it".into()
                );
            }
            self.revealed.push(identity);
        }
        if matches!(observation, DesktopStartupObservation::NativeRetired(_)) {
            if prepared.is_some_and(|value| !value.released) {
                return Err("native source loan still held".into());
            }
            self.retired = true;
        }
        Ok(())
    }
    fn system_accent(
        &mut self,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    ) -> BackendResult<()> {
        self.accents.push(accent);
        Ok(())
    }
    #[cfg(feature = "native-composition")]
    fn canvas_admission(
        &self,
        _: MainIdentity,
        _: &MemoryLease,
    ) -> Result<(), crate::startup::CanvasRefusal> {
        Err(crate::startup::CanvasRefusal::PanelNotConfigured)
    }
    #[cfg(all(windows, feature = "native-composition"))]
    fn canvas_backend(&self, _: MainIdentity) -> Option<crate::startup::CanvasBackend> {
        None
    }
    #[cfg(windows)]
    fn workspace_context(
        &self,
        _: &Self::Prepared,
    ) -> BackendResult<Option<super::WorkspaceContext>> {
        Ok(None)
    }
    #[cfg(windows)]
    fn configure_workspace(
        &mut self,
        _: sugarsubstitute_desktop::SessionBootstrap,
        _: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    ) -> BackendResult<()> {
        Ok(())
    }
    fn main_construction_pending(&self) -> bool {
        false
    }
    fn console_text(&self) -> Option<&dyn sugarsubstitute_desktop::ConsoleText> {
        None
    }
    fn transcript(&self) -> Option<crate::OutputSnapshot> {
        None
    }
    fn take_startup_event(&mut self) -> BackendResult<Option<StartupEvent>> {
        Ok(None)
    }
    fn take_command(&mut self) -> BackendResult<Option<DesktopStartupCommand>> {
        Ok(None)
    }
    fn next_deadline(&self) -> Option<Moment> {
        None
    }
    fn cleanup_failed(&self) -> bool {
        false
    }
    fn ensure_retry_ready(&self, _: MainIdentity) -> BackendResult<()> {
        Err("unsettled native owner".into())
    }
    fn retry_attempt(&mut self, _: MainIdentity, _: &mut Prepared) -> BackendResult<()> {
        Err("retry must remain fenced".into())
    }
    fn advance_reload(&mut self, _: &mut Prepared) -> BackendResult<()> {
        self.reload_failure.take().map_or(Ok(()), Err)
    }
    fn begin_workspace_close(&mut self) -> BackendResult<ShutdownInitiation> {
        Ok(self.budget.map_or(ShutdownInitiation::NoCapture, ShutdownInitiation::Captured))
    }
    fn begin_shutdown(&mut self) -> BackendResult<ShutdownInitiation> {
        self.shutdown_starts += 1;
        Ok(self.budget.map_or(ShutdownInitiation::NoCapture, ShutdownInitiation::Captured))
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.budget
    }
    fn update_status(&self) -> Option<&sugarsubstitute_contracts::update::status::UpdateStatus> {
        self.update_status.as_ref()
    }
    fn workflow_queue(&self) -> sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue {
        sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue::Unobserved
    }
    fn take_update_notice(
        &mut self,
    ) -> Option<sugarsubstitute_contracts::update::status::StartupNotice> {
        self.notices.pop_front()
    }
    fn report_page_failure(
        &mut self,
        failure: sugarsubstitute_supervision::lifecycle_protocol::PageFailure,
    ) -> BackendResult<()> {
        self.page_failures.push(failure);
        Ok(())
    }
    fn update_report_export(
        &mut self,
        text: String,
    ) -> Option<Box<dyn sugarsubstitute_desktop::report::surface::UpdateExport>> {
        self.exported.push(text);
        None
    }
    fn request_restart_to_apply(&mut self) -> BackendResult<()> {
        self.restart_requests += 1;
        Ok(())
    }
    fn request_shutdown(&mut self, budget: ShutdownBudget) -> BackendResult<()> {
        if self.budget.is_some_and(|original| original != budget) {
            return Err("renewed shutdown budget".into());
        }
        self.budget = Some(budget);
        self.shutdown_requests.push(budget);
        Ok(())
    }
    fn poll_shutdown(&mut self) -> BackendResult<bool> {
        Ok(self.retired && self.budget.is_some())
    }
    fn finish_shutdown(&mut self) -> BackendResult<()> {
        if !self.retired {
            return Err("native resources still live".into());
        }
        self.finished = true;
        Ok(())
    }
    fn startup_cause_code(_: &(dyn Error + 'static)) -> StartupCauseCode {
        StartupCauseCode::Opaque
    }
    fn close_diagnostic(
        &self,
        cause: &(dyn Error + 'static),
    ) -> (StartupCaptureState, StartupCauseCode) {
        (StartupCaptureState::Absent, Self::startup_cause_code(cause))
    }
    #[cfg(feature = "k1-native-qualification")]
    fn preview_after_main_revealed(&mut self, _: MainIdentity, _: &StartupSession) {}
    #[cfg(feature = "k1-native-qualification")]
    fn accept_preview_observation(&mut self, _: &mut Option<DesktopStartupObservation>) -> bool {
        false
    }
}

/// The reported accent reaches SugarCubes' owner and the splash view created afterwards.
#[test]
fn host_forwards_system_accent_and_applies_it_to_a_later_view() -> BackendResult<()> {
    use sugarsubstitute_contracts::appearance::AccentColor;
    use sugarsubstitute_ui::accent::AccentPalette;
    let mut host = host()?;
    let blue = Some(AccentColor::new(0, 120, 212));
    host.system_accent(blue)?;
    host.system_accent(None)?;
    host.system_accent(blue)?;
    assert_eq!(host.child.accents, vec![blue, None, blue]);
    let view =
        view::StartupView::new(Presentation::new("en", Theme::Dark, true)?, host.accent.palette());
    assert_eq!(view.appearance.accent, AccentPalette::system(AccentColor::new(0, 120, 212)));
    host.view = Some(view);
    host.system_accent(None)?;
    assert_eq!(host.view.as_ref().map(|view| view.appearance.accent), Some(AccentPalette::PRODUCT));
    Ok(())
}

#[test]
fn host_reports_main_reveal_only_after_frontend_readiness_and_never_after_close()
-> BackendResult<()> {
    for closing in [false, true] {
        let mut host = host()?;
        let launch = host.launch_id();
        let mut session = StartupSession::new(
            launch,
            Generation::new(1)?,
            StartupConfiguration::new(
                ConnectionConfiguration::Attach(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                ProfileId::new("reveal-test".into())?,
                Presentation::new("en", Theme::Dark, true)?,
            ),
        );
        host.startup = Some(Prepared { session, released: false });
        let identity = host.main_identity().ok_or("Main identity absent")?;
        assert!(host.observe(DesktopStartupObservation::MainRevealed(identity)).is_err());
        assert!(host.child.revealed.is_empty());
        session = host.startup.take().ok_or("Startup absent")?.session;
        let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
        for observation in [
            StartupObservation::SplashPresented,
            StartupObservation::ConnectionReady(origin),
            StartupObservation::FrontendNavigationStarted(identity.navigation),
        ] {
            session.observe(StartupEvent::new(launch, observation))?;
        }
        let receipt = ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"ready","evidence":{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}"#)?;
        session.observe(StartupEvent::new(launch, StartupObservation::FrontendReceipt(receipt)))?;
        session.observe(StartupEvent::new(launch, StartupObservation::SplashRetired))?;
        host.startup = Some(Prepared { session, released: false });
        if closing {
            host.request_shutdown()?;
        }
        host.observe(DesktopStartupObservation::MainRevealed(identity))?;
        assert_eq!(host.child.revealed, if closing { vec![] } else { vec![identity] });
        host.request_shutdown()?;
        host.observe(DesktopStartupObservation::NativeRetired(host.run_key()))?;
        host.finish_shutdown()?;
    }
    Ok(())
}
pub(super) fn host() -> BackendResult<StartupHost<Platform>> {
    host_with_clock(MonotonicClock::new())
}
/// The same inert host on a caller-owned clock.
pub(super) fn host_with_clock(
    clock: Arc<dyn super::HostClock>,
) -> BackendResult<StartupHost<Platform>> {
    let run = policy::tests::test_run()?;
    StartupHost::new(
        run,
        LaunchId::new(1)?,
        Platform::default(),
        clock,
        JobSequence::new(),
        StartupScopes {
            child: ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            render: ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        },
    )
}

#[path = "platform_tests/consume.rs"]
pub(super) mod consume;
#[path = "platform_tests/update_notices.rs"]
mod update_notices;

#[cfg(windows)]
#[test]
fn application_canvas_requires_a_new_scope_and_a_local_cache_anchor() -> BackendResult<()> {
    let mut host = host()?;
    let root = tempfile::tempdir()?;
    let health = crate::diagnostics::DiagnosticHealth::fixture();
    for scope in [
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        ScopeSpec { id: ScopeId::new(3)?, deadline: Some(Moment::from_nanos(1)) },
    ] {
        assert!(host.configure_canvas(root.path().to_path_buf(), health.clone(), scope).is_err());
        assert!(host.canvas.is_none());
    }
    let valid = ScopeSpec { id: ScopeId::new(3)?, deadline: None };
    assert!(host.configure_canvas("relative".into(), health.clone(), valid).is_err());
    host.configure_canvas(root.path().to_path_buf(), health.clone(), valid)?;
    assert!(host.configure_canvas(root.path().to_path_buf(), health, valid).is_err());
    Ok(())
}
#[test]
fn platform_neutral_host_preserves_source_guard_until_actual_native_retirement() -> BackendResult<()>
{
    let mut host = host()?;
    let launch = host.launch_id();
    host.startup = Some(Prepared {
        session: StartupSession::new(
            launch,
            Generation::new(1)?,
            StartupConfiguration::new(
                ConnectionConfiguration::Attach(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                ProfileId::new("portable-test".into())?,
                Presentation::new("en", Theme::Dark, true)?,
            ),
        ),
        released: false,
    });
    host.observe(DesktopStartupObservation::SplashPresented(launch))?;
    assert_eq!(host.child.effects, ["prepare"]);
    host.request_shutdown()?;
    let original = host.child.budget;
    assert_eq!(host.child.effects, ["prepare", "cleanup"]);
    host.observe(DesktopStartupObservation::SplashRetired(launch))?;
    assert!(!host.startup.as_ref().ok_or("guard missing")?.released);
    assert!(!host.poll_shutdown()?);
    assert!(host.finish_shutdown().is_err());
    host.observe(DesktopStartupObservation::NativeRetired(host.run_key()))?;
    assert!(host.startup.as_ref().ok_or("guard missing")?.released);
    assert_eq!(host.child.budget, original);
    assert!(host.poll_shutdown()?);
    host.finish_shutdown()?;
    assert!(host.child.finished);
    Ok(())
}
#[test]
fn platform_neutral_retry_rejection_keeps_the_same_application_attempt() -> BackendResult<()> {
    let mut host = host()?;
    let mut prepared = Prepared {
        session: StartupSession::new(
            LaunchId::new(1)?,
            Generation::new(1)?,
            StartupConfiguration::new(
                ConnectionConfiguration::Attach(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                ProfileId::new("portable-test".into())?,
                Presentation::new("en", Theme::Dark, true)?,
            ),
        ),
        released: false,
    };
    let identity = MainIdentity {
        run: host.run_key(),
        launch: LaunchId::new(2)?,
        navigation: Generation::new(2)?,
    };
    assert!(
        StartupHost::prepare_retry_policy(&host.child, &mut prepared.session, identity).is_err()
    );
    assert_eq!(prepared.session.launch(), LaunchId::new(1)?);
    assert_eq!(prepared.session.phase(), StartupPhase::SplashFirstFrame);
    host.request_shutdown()?;
    host.observe(DesktopStartupObservation::NativeRetired(host.run_key()))?;
    host.finish_shutdown()?;
    Ok(())
}
