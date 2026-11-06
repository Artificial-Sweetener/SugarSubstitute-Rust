// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the main window's canvas in the existing application execution lifetime.

use crate::{
    diagnostics::DiagnosticHealth,
    job_sequence::JobSequence,
    native_canvas::{
        config::{Config, InitialSource},
        gpu_policy::EmbeddedGpuPolicy,
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
#[cfg(feature = "native-composition")]
pub(super) use surface::RoutedInput;
#[cfg(test)]
mod tests;

/// The input canvas's session directory inside the installation's `user` folder, which holds
/// user data that every installation change preserves.
#[cfg(feature = "native-composition")]
const INPUT_DOCUMENTS: &str = "input-canvas";

/// Canvas storage the application's execution capacity funds.
#[derive(Clone, Copy)]
pub(super) struct CanvasFunding {
    /// Largest canvas extent, in physical pixels.
    pub(super) maximum_extent: Extent,
    /// Storage for recently shown output documents kept for instant reselection, in bytes.
    pub(super) retained_document_bytes: u64,
}

/// One application-owned, initially empty canvas; the desktop owns its eventual native surface.
pub(super) struct EmbeddedCanvas {
    anchor: PathBuf,
    health: DiagnosticHealth,
    clock: Arc<dyn Clock>,
    jobs: JobSequence,
    scope: ScopeSpec,
    funding: CanvasFunding,
    session: Option<Session>,
    /// The memory budget's latest retained-document limit, applied to every session.
    #[cfg(feature = "native-composition")]
    retained_limit: Option<u64>,
    cutoff: Option<Moment>,
    reported_ready: bool,
    /// The showing display's refresh rate, kept for a session created after it was reported.
    #[cfg(feature = "native-composition")]
    display: Option<Option<f64>>,
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
        funding: CanvasFunding,
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
            funding,
            session: None,
            #[cfg(feature = "native-composition")]
            retained_limit: None,
            cutoff: None,
            reported_ready: false,
            #[cfg(feature = "native-composition")]
            display: None,
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

    /// Whether the main lifetime's current backend grant, if any, is the one this canvas holds.
    #[cfg(feature = "native-composition")]
    pub(super) fn granted(&self, backend: Option<crate::startup::CanvasBackend>) -> bool {
        backend.is_some_and(|backend| self.matches_backend(backend))
    }

    /// Whether timed canvas work is due within a few display frames, so the event loop needs
    /// a fine system timer period to meet display-rate deadlines.
    pub(super) fn animating(&self) -> bool {
        let soon = Instant::now() + Duration::from_millis(100);
        self.next_deadline().is_some_and(|deadline| deadline <= soon)
    }

    /// Retire a closed native navigation even when it never acquired an attachment.
    #[cfg(feature = "native-composition")]
    pub(super) fn main_retired(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.retire_navigation(identity, self.session.as_mut())
    }

    /// Start once on the application's existing executor; a reload retains the same session.
    /// The renderer policy is fixed for the session; an unavailable GPU presents CPU frames.
    pub(super) fn start(
        &mut self,
        client: ExecutionClient,
        locale: &str,
        gpu: EmbeddedGpuPolicy,
    ) -> Result<(), Box<dyn Error>> {
        if self.cutoff.is_some() {
            return Err("Application canvas has closed admission".into());
        }
        if self.session.is_some() {
            return Ok(());
        }
        // Only the composition can present GPU frames; other hosts keep CPU presentation.
        #[cfg(all(windows, feature = "native-composition"))]
        let gpu = gpu.request();
        #[cfg(not(all(windows, feature = "native-composition")))]
        let gpu = {
            let _ = gpu;
            None
        };
        let config = Config {
            gpu,
            capture: false,
            verify_native: false,
            cache: sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.anchor,
            )
            .user(),
            initial: InitialSource::AwaitHost,
            locale: locale.to_owned(),
            extent: Extent::new(1, 1)?,
            maximum_extent: self.funding.maximum_extent,
            retained_document_bytes: self.funding.retained_document_bytes,
        };
        let mut session =
            Session::new(config, self.health.clone(), self.clock.clone(), self.jobs.clone())?;
        #[cfg(all(windows, feature = "native-composition"))]
        session.attach_composition_surface()?;
        #[cfg(not(all(windows, feature = "native-composition")))]
        session.attach_cpu_surface()?;
        session.input(Input::Drawable(false))?;
        #[cfg(feature = "native-composition")]
        if let Some(limit) = self.retained_limit {
            session.set_retained_limit(limit);
        }
        #[cfg(feature = "native-composition")]
        if let Some(hertz) = self.display {
            session.input(Input::DisplayRefresh(hertz))?;
        }
        #[cfg(feature = "native-composition")]
        session.persist_inputs(crate::input_documents::InputDocumentStore::new(
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.anchor,
            )
            .user()
            .join(INPUT_DOCUMENTS),
        )?);
        self.session = Some(session);
        let runtime = Runtime::shared(client, self.scope)?;
        self.session.as_mut().ok_or("Application canvas disappeared")?.start_runtime(runtime)
    }

    /// Bound retained output documents to the memory budget's native allowance, in bytes.
    #[cfg(feature = "native-composition")]
    pub(super) fn limit_retained(&mut self, bytes: u64) {
        self.retained_limit = Some(bytes);
        if let Some(session) = &mut self.session {
            session.set_retained_limit(bytes);
        }
    }

    /// Bytes retained output documents hold, as charged to the memory budget.
    #[cfg(feature = "native-composition")]
    pub(super) fn retained_bytes(&self) -> u64 {
        self.session.as_ref().map_or(0, Session::retained_bytes)
    }

    /// Advance the canvas after the application's single completion drain.
    pub(super) fn poll(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(session) = &mut self.session {
            session.poll(Duration::ZERO)?;
            #[cfg(feature = "native-composition")]
            self.surface.poll(session)?;
            #[cfg(feature = "native-composition")]
            if let Some(cutoff) = self.cutoff
                && self.surface.preview_custody_settled()
            {
                session.close_until(cutoff)?;
            }
            #[cfg(feature = "native-composition")]
            let restored = session.inputs_restored();
            #[cfg(not(feature = "native-composition"))]
            let restored = true;
            if !self.reported_ready
                && session.initialized()
                && session.renderer_settled()
                && restored
            {
                self.reported_ready = true;
                tracing::info!(
                    operation = "embedded_canvas_ready",
                    gpu = session.gpu_active(),
                    gpu_fallback = session.gpu_fallback().unwrap_or("none"),
                    "Application canvas storage is ready"
                );
            }
        }
        Ok(())
    }

    /// Receive the current panel's bounded native canvas protocol request.
    ///
    /// The surface dispatches each request family to its own entry point; see
    /// NativeSurface::native.
    #[cfg(feature = "native-composition")]
    pub(super) fn native(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        request: sugarsubstitute_contracts::canvas_native::CanvasNativeRequest,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
        storage: sugarsubstitute_desktop::CanvasNativeRequestStorage,
    ) -> Result<(), Box<dyn Error>> {
        self.admit_composition_gpu();
        let edit = matches!(
            request,
            sugarsubstitute_contracts::canvas_native::CanvasNativeRequest::Edit(_)
        );
        self.surface.native(identity, request, claim, storage, self.session.as_mut())?;
        // An edit changes the document or tools now; advance so its frame is admitted at once.
        if edit { self.poll() } else { Ok(()) }
    }

    /// Only composition-hosted main lifetimes deliver canvas messages here, so the first one
    /// admits the opted-in GPU; windowed hosting never creates a device for an unused canvas.
    #[cfg(feature = "native-composition")]
    fn admit_composition_gpu(&mut self) {
        #[cfg(windows)]
        if let Some(session) = &mut self.session {
            session.admit_composition_gpu();
        }
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
        self.admit_composition_gpu();
        match message {
            sugarsubstitute_contracts::canvas_surface::CanvasSurfaceMessage::Cancel {
                request_id,
                ..
            } => self.surface.cancel(identity, request_id, claim, self.session.as_mut()),
            message => match self.session.as_mut() {
                Some(session) => {
                    self.surface.receive(identity, message, allocation, claim, session)?;
                    // Act on the operation now, as routed input does, rather than on the event
                    // loop's next wake: a retained output reselected here presents in this pass.
                    self.poll()
                }
                None => self.surface.refuse(identity, message, claim, None),
            },
        }
    }

    /// Preserve terminal response custody when the current page lacks application authority.
    #[cfg(feature = "native-composition")]
    pub(super) fn refuse_surface(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        message: sugarsubstitute_contracts::canvas_surface::CanvasSurfaceMessage,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.refuse(identity, message, claim, self.session.as_mut())
    }

    /// Answer a native request the current page has no canvas authority for as unavailable.
    #[cfg(feature = "native-composition")]
    pub(super) fn refuse_native(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        request: &sugarsubstitute_contracts::canvas_native::CanvasNativeRequest,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        self.surface.refuse_native(identity, request, claim)
    }

    /// Transfer a correlated native canvas or surface reply with its admitted allocation.
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

    /// Describe where routed native gestures may reach the shown final image.
    #[cfg(feature = "native-composition")]
    pub(super) fn input_region(
        &self,
    ) -> Option<sugarsubstitute_desktop::canvas::MainCanvasInputRegion<'_>> {
        self.surface.input_region(self.session.as_ref()?)
    }

    /// Take the copied original of the latest routed drag-out for the desktop's OS drag.
    #[cfg(feature = "native-composition")]
    pub(super) fn take_drag_out(
        &mut self,
    ) -> Option<sugarsubstitute_desktop::canvas::MainCanvasDragOut> {
        self.surface.take_drag_out(self.session.as_mut()?)
    }

    /// Apply one gesture the desktop routed into the published native input region, then
    /// advance at once so its frame is admitted on this turn rather than after the next wake.
    #[cfg(feature = "native-composition")]
    pub(super) fn input(
        &mut self,
        input: sugarsubstitute_desktop::canvas::MainCanvasInput,
    ) -> Result<surface::RoutedInput, Box<dyn Error>> {
        let routed = match self.session.as_mut() {
            Some(session) => self.surface.routed_input(input, session)?,
            None => return Ok(surface::RoutedInput::Refused),
        };
        if routed == surface::RoutedInput::Applied {
            self.poll()?;
        }
        Ok(routed)
    }

    /// Follow the active display's refresh rate so navigation and presentation pace to it.
    #[cfg(feature = "native-composition")]
    pub(super) fn display(&mut self, hertz: Option<f64>) -> Result<(), Box<dyn Error>> {
        self.display = Some(hertz);
        if let Some(session) = &mut self.session {
            session.input(Input::DisplayRefresh(hertz))?;
            tracing::info!(
                operation = "canvas_display_timing",
                hertz = hertz.unwrap_or(60.0),
                "Embedded canvas display timing updated"
            );
        }
        Ok(())
    }

    /// Accept the exact native frame, resolve the matching shared operation, and admit the
    /// next frame at once: presentation released the display slot this frame held.
    #[cfg(feature = "native-composition")]
    pub(super) fn presented(
        &mut self,
        target: sugarsubstitute_canvas::FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.session.as_mut().ok_or("Application canvas is not started")?;
        self.surface.presented(target, session)?;
        self.poll()
    }

    /// Resolve hidden geometry only after the matching native hide completed in composition.
    #[cfg(feature = "native-composition")]
    pub(super) fn hidden(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        match self.session.as_mut() {
            Some(session) => self.surface.hidden(identity, session),
            None => Ok(()),
        }
    }

    /// Borrow its actual refinement deadline without creating a timer.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let native = self.session.as_ref().and_then(Lifetime::next_deadline);
        #[cfg(feature = "native-composition")]
        {
            let preview = self.surface.preview_deadline(self.clock.now());
            let editing = self.surface.editing_deadline(self.clock.now());
            let viewport = self.surface.viewport_deadline(self.clock.now());
            [native, preview, editing, viewport].into_iter().flatten().min()
        }
        #[cfg(not(feature = "native-composition"))]
        native
    }

    /// Fence the session under the same original shutdown budget as the application.
    pub(super) fn close_until(&mut self, cutoff: Moment) -> Result<(), Box<dyn Error>> {
        if self.cutoff.is_some_and(|original| original <= cutoff) {
            return Ok(());
        }
        #[cfg(feature = "native-composition")]
        let cutoff = self
            .session
            .as_mut()
            .map_or(cutoff, |session| session.adopt_preview_retirement_cutoff(cutoff));
        self.cutoff = Some(cutoff);
        #[cfg(feature = "native-composition")]
        self.surface.close(self.session.as_mut())?;
        if let Some(session) = &mut self.session {
            #[cfg(feature = "native-composition")]
            if !self.surface.preview_custody_settled() {
                return Ok(());
            }
            session.close_until(cutoff)?;
        }
        Ok(())
    }

    /// A requested cancellation is not settlement; retain all work until it actually stops.
    pub(super) fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        if let Some(session) = &mut self.session
            && let Some(cutoff) = self.cutoff
        {
            self.surface.poll(session)?;
            if !self.surface.preview_custody_settled() {
                return Ok(false);
            }
            session.close_until(cutoff)?;
        }
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
