// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the main window's canvas in the existing application execution lifetime.

use crate::{
    diagnostics::DiagnosticHealth,
    job_sequence::JobSequence,
    native_canvas::{
        config::{Config, InitialSource},
        session::{Session, runtime::Runtime},
    },
};
use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, ScopeSpec};
use sugarsubstitute_desktop::canvas::{Input, Lifetime};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_image_ops::Extent;

#[cfg(feature = "native-composition")]
mod comfy_source;
#[cfg(feature = "native-composition")]
mod surface;
#[cfg(test)]
mod tests;

/// One application-owned, initially empty canvas; the desktop owns its eventual native surface.
pub(super) struct EmbeddedCanvas {
    anchor: PathBuf,
    health: DiagnosticHealth,
    clock: Arc<dyn Clock>,
    jobs: JobSequence,
    scope: ScopeSpec,
    session: Option<Session>,
    cutoff: Option<Moment>,
    reported_ready: bool,
    #[cfg(feature = "native-composition")]
    surface: surface::NativeSurface,
}

impl EmbeddedCanvas {
    /// Keep configuration inert until the accepted main window is revealed.
    #[cfg(any(feature = "windows-native-host", test))]
    pub(super) fn new(
        anchor: PathBuf,
        health: DiagnosticHealth,
        clock: Arc<dyn Clock>,
        jobs: JobSequence,
        scope: ScopeSpec,
    ) -> Self {
        #[cfg(all(feature = "native-composition", test))]
        let surface = surface::NativeSurface::fixture(anchor.join("canvas-qualification-output"));
        #[cfg(all(feature = "native-composition", not(test)))]
        let surface = surface::NativeSurface::new();
        Self {
            anchor,
            health,
            clock,
            jobs,
            scope,
            session: None,
            cutoff: None,
            reported_ready: false,
            #[cfg(feature = "native-composition")]
            surface,
        }
    }

    /// Update the exact authenticated backend and fence any superseded acquisition.
    #[cfg(feature = "native-composition")]
    pub(super) fn backend(
        &mut self,
        backend: Option<crate::startup::CanvasBackend>,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.backend(backend, self.session.as_mut())?;
        if let Some(session) = &mut self.session {
            self.surface.poll(session)?;
        }
        Ok(())
    }

    /// Check that a frame still belongs to the current authenticated backend grant.
    #[cfg(feature = "native-composition")]
    pub(super) fn matches_backend(&self, backend: crate::startup::CanvasBackend) -> bool {
        self.surface.matches_backend(backend)
    }

    /// Start once on the application's existing executor; a reload retains the same session.
    pub(super) fn start(
        &mut self,
        client: ExecutionClient,
        locale: &str,
    ) -> Result<(), Box<dyn Error>> {
        if self.cutoff.is_some() {
            return Err("Application canvas has closed admission".into());
        }
        if self.session.is_some() {
            return Ok(());
        }
        let config = Config {
            gpu: None,
            capture: false,
            verify_native: false,
            cache: self.anchor.clone(),
            initial: InitialSource::AwaitHost,
            locale: locale.to_owned(),
            extent: Extent::new(1, 1)?,
        };
        let mut session =
            Session::new(config, self.health.clone(), self.clock.clone(), self.jobs.clone())?;
        session.attach_cpu_surface()?;
        session.input(Input::Drawable(false))?;
        self.session = Some(session);
        let runtime = Runtime::shared(client, self.scope)?;
        self.session.as_mut().ok_or("Application canvas disappeared")?.start_runtime(runtime)
    }

    /// Advance the canvas after the application's single completion drain.
    pub(super) fn poll(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(session) = &mut self.session {
            session.poll(Duration::ZERO)?;
            #[cfg(feature = "native-composition")]
            self.surface.poll(session)?;
            if !self.reported_ready && session.initialized() {
                self.reported_ready = true;
                tracing::info!(
                    operation = "embedded_canvas_ready",
                    "Application canvas storage is ready"
                );
            }
        }
        Ok(())
    }

    /// Receive the current panel's bounded context request for the native surface.
    #[cfg(feature = "native-composition")]
    pub(super) fn context(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        request: sugarsubstitute_contracts::canvas_context::CanvasContextRequest,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.context(identity, request, claim, self.session.as_mut())
    }

    /// Receive the current panel's shared surface operation.
    #[cfg(feature = "native-composition")]
    pub(super) fn surface(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        message: sugarsubstitute_contracts::canvas_surface::CanvasSurfaceMessage,
        allocation: Option<sugarsubstitute_desktop::canvas::placement::WebViewAllocation>,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.session.as_mut().ok_or("Application canvas is not started")?;
        self.surface.receive(identity, message, allocation, claim, session)
    }

    /// Transfer a correlated context or surface reply with its admitted allocation.
    #[cfg(feature = "native-composition")]
    pub(super) fn take_command(
        &mut self,
    ) -> Option<sugarsubstitute_desktop::DesktopStartupCommand> {
        self.surface.take_command()
    }

    /// Distinguish a pending source from withdrawal of the authorized native surface.
    #[cfg(feature = "native-composition")]
    pub(super) fn presentation(
        &self,
    ) -> sugarsubstitute_desktop::canvas::MainCanvasPresentation<'_> {
        self.session
            .as_ref()
            .map_or(sugarsubstitute_desktop::canvas::MainCanvasPresentation::Hide, |session| {
                self.surface.presentation(session)
            })
    }

    /// Accept the exact native frame and resolve the matching shared operation.
    #[cfg(feature = "native-composition")]
    pub(super) fn presented(
        &mut self,
        target: sugarsubstitute_canvas::FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.session.as_mut().ok_or("Application canvas is not started")?;
        self.surface.presented(target, session)
    }

    /// Resolve hidden geometry only after the matching native hide completed in composition.
    #[cfg(feature = "native-composition")]
    pub(super) fn hidden(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.hidden(identity)
    }

    /// Borrow its actual refinement deadline without creating a timer.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.session.as_ref().and_then(Lifetime::next_deadline)
    }

    /// Fence the session under the same original shutdown budget as the application.
    pub(super) fn close_until(&mut self, cutoff: Moment) -> Result<(), Box<dyn Error>> {
        if self.cutoff.is_some_and(|original| original <= cutoff) {
            return Ok(());
        }
        self.cutoff = Some(cutoff);
        #[cfg(feature = "native-composition")]
        self.surface.close();
        if let Some(session) = &mut self.session {
            session.close_until(cutoff)?;
        }
        Ok(())
    }

    /// A requested cancellation is not settlement; retain all work until it actually stops.
    pub(super) fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        match &mut self.session {
            None => Ok(true),
            Some(_) if self.cutoff.is_none() => Ok(false),
            Some(session) => session.settled(),
        }
    }

    /// Release only the already-settled canvas; the application owns the executor join.
    pub(super) fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(session) = &mut self.session {
            session.finish()?;
        }
        self.session = None;
        Ok(())
    }
}
