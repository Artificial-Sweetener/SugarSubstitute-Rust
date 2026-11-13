// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual child startup, application policy and governed splash preparation.
//!
//! This owner stays on the actual native event-loop thread. The child retains real
//! authenticated bootstrap, capture, readiness and profile custody. Matching data,
//! prepared pixels or an execution terminal cannot substitute for those capabilities.

#[cfg(windows)]
mod canvas;
#[path = "startup/close_diagnostic.rs"]
mod close_diagnostic;
#[path = "startup/close_save.rs"]
mod close_save;
#[path = "startup/commands.rs"]
mod commands;
#[cfg(all(windows, feature = "native-composition"))]
mod composition;
#[path = "startup/configuration.rs"]
mod configuration;
#[path = "startup/consume_diagnostic.rs"]
mod consume_diagnostic;
#[path = "startup/delivery.rs"]
mod delivery;
#[path = "startup/execution.rs"]
mod execution;
#[path = "startup/lifetime.rs"]
mod lifetime;
#[cfg(all(windows, any(feature = "windows-native-host", test)))]
pub(crate) mod memory;
#[path = "startup/policy.rs"]
mod policy;
#[path = "startup/routing.rs"]
mod routing;
#[path = "startup/scopes.rs"]
mod scopes;
#[path = "startup/shutdown.rs"]
mod shutdown;
#[cfg(all(windows, any(feature = "windows-native-host", test)))]
pub(crate) mod updates_page;
#[path = "startup/view.rs"]
pub(crate) mod view;
#[path = "startup/window_content.rs"]
mod window_content;

mod appearance;
mod backend;
pub(crate) mod budget_trace;
#[cfg(windows)]
mod native_admission;
mod shutdown_origin;
use crate::{job_sequence::JobSequence, preparation::PreparationDriver};
#[cfg(all(windows, feature = "native-composition"))]
pub use backend::CanvasBackend;
#[cfg(feature = "native-composition")]
pub use backend::CanvasRefusal;
#[cfg(windows)]
pub use backend::WorkspaceContext;
pub use backend::{BackendResult, PreparedStartup, StartupBackend};
#[cfg(all(windows, feature = "native-composition"))]
mod workspace;
use execution::StartupExecution;
use policy::StartupPolicy;
pub use scopes::StartupScopes;
pub use shutdown_origin::ShutdownInitiation;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    appearance::AccentColor,
    execution::{CompletionWake, Scope},
    lifecycle::RunKey,
    presentation::{FrameShape, PreparedFrame},
    startup::LaunchId,
};
use sugarsubstitute_desktop::{
    AccessibleControl, DesktopStartupCommand, DesktopStartupObservation, HitTarget, KeyAction,
    Pointer, PointerAction, StartupPreparationStep, StartupWindowContent, WindowContent,
};
use sugarsubstitute_execution::MonotonicClock;
use view::StartupView;

#[cfg(test)]
mod platform_tests;

/// Retain one real native-thread process lifetime until actual native and worker joins.
pub struct StartupHost<B: StartupBackend> {
    #[cfg(windows)]
    canvas: Option<canvas::EmbeddedCanvas>,
    /// The application memory budget, governing once a canvas exists.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    memory: Option<memory::MemoryGovernor>,
    /// The page's update state and its check and restart answers.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    updates: updates_page::UpdatesPage,
    #[cfg(all(windows, feature = "native-composition"))]
    composition: Option<composition::CompositionWork>,
    #[cfg(all(windows, feature = "native-composition"))]
    workspace: Option<workspace::WorkspaceWork>,
    policy: StartupPolicy,
    startup: Option<B::Prepared>,
    child: B,
    execution: StartupExecution,
    clock: Arc<MonotonicClock>,
    jobs: JobSequence,
    scopes: StartupScopes,
    preparation: Option<PreparationDriver>,
    render_scope: Option<Scope>,
    child_scope: Option<Scope>,
    view: Option<StartupView>,
    accent: appearance::SystemAccent,
    visible: bool,
    frame_dirty: bool,
    started: bool,
    prepared: bool,
    closing: bool,
    finished: bool,
    commands: commands::StartupCommands,
    next_deadline: Option<Instant>,
    shutdown: shutdown::ShutdownTransaction,
    /// Why the application closes; the first close request decides it.
    close_reason: sugarsubstitute_application::workspace_session::close_save::CloseReason,
    /// The close save, once the shutdown budget was measured.
    close_save: Option<sugarsubstitute_application::workspace_session::close_save::CloseSave>,
    close_record_submitted: bool,
    /// The short graceful cutoff of a critical session end, which waits for nothing.
    critical_until: Option<sugarsubstitute_contracts::execution::Moment>,
    #[cfg(all(test, windows, feature = "native-composition"))]
    input_probe: Option<close_save::InputSaveProbe>,
}

#[cfg(all(test, windows, feature = "windows-native-host"))]
pub(crate) fn constructor_roundtrip<B: StartupBackend>(
    run: RunKey,
    launch: LaunchId,
    child: B,
    jobs: JobSequence,
) -> Result<B, Box<dyn Error>> {
    let content = StartupHost::new(
        run,
        launch,
        child,
        MonotonicClock::new(),
        jobs,
        StartupScopes {
            child: sugarsubstitute_contracts::execution::ScopeSpec {
                id: sugarsubstitute_contracts::execution::ScopeId::new(1)?,
                deadline: None,
            },
            render: sugarsubstitute_contracts::execution::ScopeSpec {
                id: sugarsubstitute_contracts::execution::ScopeId::new(2)?,
                deadline: None,
            },
        },
    )?;
    Ok(content.child)
}
impl<B: StartupBackend> StartupHost<B> {
    /// Retain inert role inputs; the real native wake starts execution exactly once.
    /// Bootstrap authentication and required capture complete only through the child owner.
    /// # Errors
    /// Rejects aliased original child/render scopes before execution or native admission.
    pub fn new(
        run: RunKey,
        launch: LaunchId,
        child: B,
        clock: Arc<MonotonicClock>,
        jobs: JobSequence,
        scopes: StartupScopes,
    ) -> Result<Self, Box<dyn Error>> {
        scopes.validate()?;
        Ok(Self {
            #[cfg(windows)]
            canvas: None,
            #[cfg(all(windows, any(feature = "windows-native-host", test)))]
            memory: None,
            // Until the coordinator's update status reaches this host, the page is told the
            // truth this host knows: up to date and idle, with no workflow to wait for.
            #[cfg(all(windows, any(feature = "windows-native-host", test)))]
            updates: updates_page::UpdatesPage::new(
                Box::new(sugarsubstitute_application::updates_panel::IdleUpdateStatus),
                Box::new(sugarsubstitute_application::updates_panel::IdleWorkflowActivity),
            )?,
            #[cfg(all(windows, feature = "native-composition"))]
            composition: None,
            #[cfg(all(windows, feature = "native-composition"))]
            workspace: None,
            policy: StartupPolicy::new(run, launch),
            startup: None,
            child,
            execution: StartupExecution::new(clock.clone()),
            clock,
            jobs,
            scopes,
            preparation: None,
            render_scope: None,
            child_scope: None,
            view: None,
            accent: appearance::SystemAccent::default(),
            visible: false,
            frame_dirty: false,
            started: false,
            prepared: false,
            closing: false,
            finished: false,
            commands: commands::StartupCommands::new(),
            next_deadline: None,
            shutdown: shutdown::ShutdownTransaction::new(),
            close_reason:
                sugarsubstitute_application::workspace_session::close_save::CloseReason::Close,
            close_save: None,
            close_record_submitted: false,
            critical_until: None,
            #[cfg(all(test, windows, feature = "native-composition"))]
            input_probe: None,
        })
    }
    fn invalidate(&mut self, changed: bool) -> bool {
        self.frame_dirty |= changed;
        if changed && let Some(render) = &mut self.preparation {
            render.invalidate();
        }
        changed
    }
}
impl<B: StartupBackend> WindowContent for StartupHost<B> {
    fn presentation_visible(&self) -> bool {
        !self.closing
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.start_owned(wake)
    }
    fn request_frame(
        &mut self,
        shape: FrameShape,
        elapsed: Duration,
    ) -> Result<(), Box<dyn Error>> {
        if !self.prepared || self.closing || self.policy.closing() {
            return Err(std::io::Error::other("Startup surface admission closed").into());
        }
        let startup = self
            .startup
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Actual startup policy absent"))?;
        let view =
            self.view.as_mut().ok_or_else(|| std::io::Error::other("Startup view absent"))?;
        view.sample(startup.session().phase(), elapsed, Instant::now());
        if self.child.cleanup_failed() {
            view.appearance.status = sugarsubstitute_splash::StartupStatus::CleanupFailed;
        }
        view.animation.tick(elapsed);
        let progress = view.progress();
        self.preparation
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Startup renderer absent"))?
            .request(
                shape,
                view.appearance,
                view.animation.frame(elapsed),
                progress,
                self.child.transcript().map(Into::into),
            )?;
        self.frame_dirty = false;
        Ok(())
    }
    fn poll_frame(&mut self, max: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let frame = self.consume(max)?;
        if frame.is_some()
            && let Some(view) = &mut self.view
        {
            let appearance =
                self.preparation.as_ref().and_then(PreparationDriver::published_appearance);
            let appearance = consume_diagnostic::published_appearance::<B>(appearance)?;
            view.publish_controls(appearance);
        }
        Ok(frame)
    }
    fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>> {
        self.visible = visible && !self.closing;
        if let Some(render) = &mut self.preparation {
            render.set_visible(self.visible)?;
        }
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.close_owned()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.poll_owned_shutdown()
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.finish_owned_shutdown()
    }
    fn hit_test(&self, pointer: Pointer) -> HitTarget {
        self.view.as_ref().map_or(HitTarget::Client, |view| view.hit_test(pointer))
    }
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool {
        let changed = self.view.as_mut().is_some_and(|view| view.pointer(action, pointer));
        self.invalidate(changed)
    }
    fn key(&mut self, action: KeyAction) -> bool {
        let changed = self.view.as_mut().is_some_and(|view| view.key(action));
        self.invalidate(changed)
    }
    fn animating(&self) -> bool {
        self.prepared
            && self.visible
            && !self.closing
            && (self.frame_dirty
                || self
                    .view
                    .as_ref()
                    .is_some_and(|view| view.motion() || view.feedback_due(Instant::now()))
                || !self.preparation.as_ref().is_some_and(PreparationDriver::enriched))
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::ConsoleView<'_>> {
        let view = self.view.as_ref()?;
        Some(view.console(self.child.console_text()))
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        self.view.as_ref().map_or(&[], StartupView::controls)
    }
    fn activate(&mut self, id: u16) -> bool {
        let changed = self.view.as_mut().is_some_and(|view| view.activate(id));
        self.invalidate(changed)
    }
}
