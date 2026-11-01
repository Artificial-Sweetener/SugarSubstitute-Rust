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
#[path = "startup/policy.rs"]
mod policy;
#[path = "startup/routing.rs"]
mod routing;
#[path = "startup/scopes.rs"]
mod scopes;
#[path = "startup/shutdown.rs"]
mod shutdown;
#[path = "startup/view.rs"]
pub(crate) mod view;

mod backend;
pub(crate) mod budget_trace;
mod shutdown_origin;
use crate::{job_sequence::JobSequence, preparation::PreparationDriver};
#[cfg(all(windows, feature = "native-composition"))]
pub use backend::CanvasBackend;
pub use backend::{BackendResult, PreparedStartup, StartupBackend};
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
    #[cfg(all(windows, feature = "native-composition"))]
    composition: Option<composition::CompositionWork>,
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
    visible: bool,
    frame_dirty: bool,
    started: bool,
    prepared: bool,
    closing: bool,
    finished: bool,
    commands: commands::StartupCommands,
    next_deadline: Option<Instant>,
    shutdown: shutdown::ShutdownTransaction,
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
            #[cfg(all(windows, feature = "native-composition"))]
            composition: None,
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
            visible: false,
            frame_dirty: false,
            started: false,
            prepared: false,
            closing: false,
            finished: false,
            commands: commands::StartupCommands::new(),
            next_deadline: None,
            shutdown: shutdown::ShutdownTransaction::new(),
        })
    }
    fn invalidate(&mut self, changed: bool) -> bool {
        self.frame_dirty |= changed;
        if changed && let Some(render) = &mut self.preparation {
            render.invalidate();
        }
        changed
    }

    /// Retain one empty application canvas before the native startup lifetime begins.
    /// `anchor` is the installation root supplied by the parent; the runtime cache owner opens
    /// its schema-qualified scope below it and validates it on a worker.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    pub(crate) fn configure_canvas(
        &mut self,
        anchor: std::path::PathBuf,
        health: crate::diagnostics::DiagnosticHealth,
        scope: sugarsubstitute_contracts::execution::ScopeSpec,
    ) -> Result<(), Box<dyn Error>> {
        if self.started
            || self.canvas.is_some()
            || scope.deadline.is_some()
            || scope.id <= self.scopes.child.id
            || scope.id <= self.scopes.render.id
        {
            return Err("Application canvas has an invalid or already active scope".into());
        }
        #[cfg(feature = "native-composition")]
        if self.composition.as_ref().is_some_and(|composition| scope.id <= composition.scope_id()) {
            return Err("Application canvas scope precedes composition execution".into());
        }
        if !anchor.is_absolute() {
            return Err("Application canvas cache anchor must be absolute".into());
        }
        self.execution.reserve_canvas_capacity()?;
        self.canvas = Some(canvas::EmbeddedCanvas::new(
            anchor,
            health,
            self.clock.clone(),
            self.jobs.clone(),
            scope,
        ));
        Ok(())
    }

    /// Reserve the composition-wait scope before native window construction begins.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn configure_composition(
        &mut self,
        scope: sugarsubstitute_contracts::execution::ScopeSpec,
    ) -> Result<(), Box<dyn Error>> {
        if self.started
            || self.composition.is_some()
            || self.canvas.is_some()
            || scope.deadline.is_some()
            || scope.id <= self.scopes.child.id
            || scope.id <= self.scopes.render.id
        {
            return Err("Composition wait scope is invalid or already active".into());
        }
        self.composition = Some(composition::CompositionWork::new(scope, self.jobs.clone()));
        Ok(())
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
            let appearance = self
                .preparation
                .as_ref()
                .and_then(PreparationDriver::published_appearance)
                .ok_or("Published startup frame has no retained appearance")?;
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

    fn main_canvas_animating(&self) -> bool {
        self.canvas.as_ref().is_some_and(|canvas| canvas.animating())
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
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(command) = self.canvas.as_mut().and_then(canvas::EmbeddedCanvas::take_command) {
            return Ok(Some(command));
        }
        self.child.take_command()
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
            .into_iter()
            .chain({
                #[cfg(windows)]
                {
                    self.canvas.as_ref().and_then(canvas::EmbeddedCanvas::next_deadline)
                }
                #[cfg(not(windows))]
                {
                    None
                }
            })
            .chain(
                self.view
                    .as_ref()
                    .filter(|_| self.visible && !self.closing)
                    .and_then(StartupView::feedback_deadline),
            )
            .min()
    }
}
