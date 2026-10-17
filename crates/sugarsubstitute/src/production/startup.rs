// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual child startup, application policy and governed splash preparation.
//!
//! This owner stays on the actual native event-loop thread. The child retains real
//! authenticated bootstrap, capture, readiness and profile custody. Matching data,
//! prepared pixels or an execution terminal cannot substitute for those capabilities.

#[path = "startup/close_diagnostic.rs"]
mod close_diagnostic;
#[path = "startup/commands.rs"]
mod commands;
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
mod view;

use super::child::{PreparedChildStartup, ProductionChild};
use crate::{job_sequence::JobSequence, preparation::PreparationDriver};
use execution::StartupExecution;
use policy::StartupPolicy;
pub(crate) use scopes::StartupScopes;
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

/// Retain one real native-thread process lifetime until actual native and worker joins.
pub(crate) struct ProductionStartup {
    policy: StartupPolicy,
    startup: Option<PreparedChildStartup>,
    child: ProductionChild,
    execution: StartupExecution,
    clock: Arc<MonotonicClock>,
    jobs: JobSequence,
    scopes: StartupScopes,
    preparation: Option<PreparationDriver>,
    render_scope: Option<Scope>,
    child_scope: Option<Scope>,
    view: Option<StartupView>,
    visible: bool,
    started: bool,
    prepared: bool,
    closing: bool,
    finished: bool,
    commands: commands::StartupCommands,
    next_deadline: Option<Instant>,
    shutdown: shutdown::ShutdownTransaction,
}

#[cfg(test)]
pub(in crate::production) fn constructor_roundtrip(
    run: RunKey,
    launch: LaunchId,
    child: ProductionChild,
    jobs: JobSequence,
) -> Result<ProductionChild, Box<dyn Error>> {
    let content = ProductionStartup::new(
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
impl ProductionStartup {
    /// Retain inert role inputs; the real native wake starts execution exactly once.
    /// Bootstrap authentication and required capture complete only through the child owner.
    /// # Errors
    /// Rejects aliased original child/render scopes before execution or native admission.
    pub(crate) fn new(
        run: RunKey,
        launch: LaunchId,
        child: ProductionChild,
        clock: Arc<MonotonicClock>,
        jobs: JobSequence,
        scopes: StartupScopes,
    ) -> Result<Self, Box<dyn Error>> {
        scopes.validate()?;
        Ok(Self {
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
        if changed && let Some(render) = &mut self.preparation {
            render.invalidate();
        }
        changed
    }
}
impl WindowContent for ProductionStartup {
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
        view.phase(startup.session().phase());
        if self.child.cleanup_failed() {
            view.appearance.status = sugarsubstitute_splash::StartupStatus::CleanupFailed;
        }
        view.elapsed = elapsed;
        view.animation.tick(elapsed);
        let progress = view.progress(startup.session().phase());
        self.preparation
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Startup renderer absent"))?
            .request(
                shape,
                view.appearance,
                view.animation.frame(elapsed),
                progress,
                self.child.transcript().map(Into::into),
            )
    }
    fn poll_frame(&mut self, max: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let frame = self.consume(max)?;
        if frame.is_some()
            && let Some(view) = &mut self.view
        {
            view.publish_controls();
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
            && (self.view.as_ref().is_some_and(StartupView::motion)
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
impl StartupWindowContent for ProductionStartup {
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
        #[cfg(feature = "k1-native-qualification")]
        let revealed = match &observation {
            DesktopStartupObservation::MainRevealed(identity) => Some(*identity),
            _ => None,
        };
        let session = self.startup.as_mut().map(PreparedChildStartup::session_mut);
        let route = self.policy.native(observation, session)?;
        self.route(route)?;
        #[cfg(feature = "k1-native-qualification")]
        if let Some(identity) = revealed
            && let Some(startup) = &self.startup
        {
            self.child.preview_after_main_revealed(identity, startup.session());
        }
        self.invalidate(true);
        self.refresh_deadline()
    }
    fn take_command(&mut self) -> Result<Option<DesktopStartupCommand>, Box<dyn Error>> {
        if let Some(command) = self.commands.take() {
            return Ok(Some(command));
        }
        self.child.take_command()
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
    }
}
