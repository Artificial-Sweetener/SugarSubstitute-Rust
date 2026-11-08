// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate the shared web surface with the current authorized canvas and backend.

mod attachment;
mod backdrops;
mod capabilities;
mod editing;
#[cfg(test)]
mod fixture_file;
mod geometry;
mod ingress;
mod input;
mod input_protocol;
mod lifecycle;
mod native;
mod navigation;
mod operations;
mod overview;
mod presentation;
mod preview;
#[cfg(test)]
#[path = "surface/replies_tests.rs"]
mod replies_tests;
mod sources;
mod viewport;
mod workflow;

use super::*;
pub(in crate::startup) use input::RoutedInput;
#[cfg(test)]
use std::path::{Component, Path};
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_comfy::output_media::ComfyViewTarget;
use sugarsubstitute_contracts::{
    canvas_native::CanvasContextRequest,
    canvas_surface::{
        CanvasFileStorage, CanvasSurfaceAddress, CanvasSurfaceMessage, CanvasSurfaceOperation,
        CanvasSurfaceReplyStatus, CanvasSurfaceRequest,
    },
    execution::MemoryClaim,
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{
    CanvasNativeReply, DesktopStartupCommand, SurfaceReply,
    canvas::{
        MainCanvasFrame, MainCanvasRetention,
        placement::{
            CanvasPlacement, PhysicalRect, WebViewAllocation, occlusions, placement,
            preparation_viewport,
        },
    },
};
use sugarsubstitute_image_io::CacheReuse;
use sugarsubstitute_image_io::ImageSource;
pub(in crate::startup) use workflow::OutputReplay;

struct Context {
    identity: MainIdentity,
    request: CanvasContextRequest,
    claim: MemoryClaim,
}

struct Binding {
    document: Option<crate::native_canvas::session::DocumentToken>,
    identity: MainIdentity,
    backend: Option<crate::startup::CanvasBackend>,
    address: CanvasSurfaceAddress,
    revision: u64,
    placement: Option<CanvasPlacement>,
    allocation: Option<WebViewAllocation>,
    /// Physical presentation regions for this attachment, retained through candidate rollback.
    backdrops: Vec<sugarsubstitute_canvas_render::backdrop::BackdropRegion>,
    /// Session-owned geometry/material generation that the next matching frame must present.
    backdrop_generation: u64,
    /// Page-published web content above the viewport, in physical client pixels.
    occlusions: Vec<PhysicalRect>,
    /// A page modal blocks the whole viewport, so no native input region is published.
    modal: bool,
    selected: bool,
    selection_revision: u64,
    source_intent_revision: Option<u64>,
    presented_selection_revision: u64,
    presented_viewport: Option<sugarsubstitute_contracts::canvas_surface::CanvasViewportState>,
    /// The current selection is a composed overview grid, whose camera stays at Fit.
    overview: bool,
    /// The presented selection was an overview grid.
    presented_overview: bool,
    committed: bool,
    presented: Option<MainCanvasRetention>,
    /// The presented pixels show the input document the Input route showed, so they are
    /// retained only while the binding still shows a document.
    presented_input: bool,
    /// Filename leaf of the selected single output, which a drag-out exports; none for an
    /// overview or comparison selection.
    drag_name: Option<String>,
    /// The output selection this attachment last offered. It ends with the attachment, so the
    /// Output route of a later attachment never replays a selection made on another one.
    offered: Option<workflow::OfferedOutput>,
}

impl Binding {
    /// Whether this binding shows a document: its selected output, or the input document the
    /// Input route shows, which needs no output selection. Presentation, geometry waits, camera
    /// commands and routed input all treat either as the binding's shown content; previews and
    /// overview grids keep their own rules.
    fn shows_document(&self, session: &Session) -> bool {
        self.selected || session.input_shown()
    }
}

#[derive(Clone, Copy)]
enum PendingKind {
    Prepare,
    Restore,
    Select,
    Geometry,
    Hide,
    Commit,
    EmptyCommit,
    /// A shared viewport command or read, answered once its camera settles; dmitted is the
    /// execution clock reading that starts its settle budget.
    Viewport {
        intent: crate::native_canvas::session::CameraIntent,
        commanded: bool,
        admitted: sugarsubstitute_contracts::execution::Moment,
    },
}

struct Pending {
    identity: MainIdentity,
    request: Box<CanvasSurfaceRequest>,
    claim: MemoryClaim,
    kind: PendingKind,
    /// The last recorded reason a pending selection's or viewport request's reply condition
    /// was unmet.
    wait: crate::canvas_surface_diagnostic::Wait,
    preparation: Option<crate::canvas_preparation_diagnostic::NativePreparationSnapshot>,
}

/// Build the host acquisition adapter for an overview of qualification fixture outputs.
#[cfg(test)]
pub(super) type FixtureAcquisition = Box<
    dyn Fn(
        crate::native_canvas::session::OverviewSelection<PathBuf>,
    ) -> Arc<dyn crate::native_canvas::session::acquisition::SourceAcquirer>,
>;

/// One native attachment and bounded replies for the current panel.
pub(super) struct NativeSurface {
    backend: Option<crate::startup::CanvasBackend>,
    pub(in crate::startup::canvas) session_source: Option<super::session_source::SessionSource>,
    #[cfg(test)]
    fixture_output: Option<PathBuf>,
    #[cfg(test)]
    fixture_acquisition: Option<FixtureAcquisition>,
    /// One-use qualification adapters that replace a named fixture file's direct offer.
    #[cfg(test)]
    fixture_acquirers:
        Vec<(String, Arc<dyn crate::native_canvas::session::acquisition::SourceAcquirer>)>,
    binding: Option<Binding>,
    retained: Option<Binding>,
    restoring: bool,
    pending: Option<Pending>,
    contexts: [Option<Context>; 4],
    replies: [Option<DesktopStartupCommand>; 8],
    preview: preview::Preview,
    /// Workflow-surface requests awaiting their input work, and the output to return to.
    workflow: workflow::Workflow,
    /// Captures awaiting their jobs and the `source.changed` subscription.
    sources: sources::Sources,
    editing: editing::Editing,
    /// The latest routed drag-out awaiting its copied original.
    drag_out: Option<input::DragRequest>,
    drag_sequence: u64,
    /// A rejected feature schema fences this navigation's later surface mutations.
    rejected_schema: Option<MainIdentity>,
    closed: bool,
}

impl NativeSurface {
    /// Begin without a backend; only the authenticated startup owner can provide one.
    pub(super) fn new() -> Self {
        Self {
            backend: None,
            session_source: None,
            #[cfg(test)]
            fixture_output: None,
            #[cfg(test)]
            fixture_acquisition: None,
            #[cfg(test)]
            fixture_acquirers: Vec::new(),
            binding: None,
            retained: None,
            restoring: false,
            pending: None,
            contexts: std::array::from_fn(|_| None),
            replies: std::array::from_fn(|_| None),
            preview: preview::Preview::default(),
            workflow: workflow::Workflow::default(),
            sources: sources::Sources::default(),
            editing: editing::Editing::default(),
            drag_out: None,
            drag_sequence: 0,
            rejected_schema: None,
            closed: false,
        }
    }

    /// Keep local file fixtures confined to deterministic canvas-unit qualification.
    #[cfg(test)]
    pub(super) fn fixture(output: PathBuf) -> Self {
        let mut surface = Self::new();
        surface.fixture_output = Some(output);
        surface
    }

    /// Acquire later overview fixtures through a host adapter instead of explicit file originals.
    #[cfg(test)]
    pub(super) fn acquire_overviews_with(&mut self, acquisition: FixtureAcquisition) {
        self.fixture_acquisition = Some(acquisition);
    }

    fn available(&self, identity: MainIdentity) -> bool {
        #[cfg(test)]
        if self.fixture_output.is_some() {
            return true;
        }
        self.backend.is_some_and(|backend| backend.identity == identity)
    }

    /// Match presentation to the backend grant that admitted the attachment.
    pub(super) fn matches_backend(&self, backend: crate::startup::CanvasBackend) -> bool {
        self.binding.as_ref().is_some_and(|binding| binding.backend == Some(backend))
    }

    /// Fence the old attachment before accepting a different or unavailable backend grant.
    /// Retained output documents belong to the previous backend's outputs and are retired.
    pub(super) fn backend(
        &mut self,
        backend: Option<crate::startup::CanvasBackend>,
        mut session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.backend == backend {
            return Ok(());
        }
        if let Some(session) = session.as_deref_mut() {
            session.forget_retained();
        }
        let changed_attachment = self
            .binding
            .as_ref()
            .is_some_and(|binding| binding.backend.is_some() && binding.backend != backend);
        self.backend = backend;
        if changed_attachment {
            self.discard_attachment(session)?;
            if let Some(pending) = self.pending.take() {
                self.reply(
                    pending.identity,
                    &pending.request,
                    CanvasSurfaceReplyStatus::Unavailable,
                    pending.claim,
                )?;
            }
        }
        Ok(())
    }

    /// Delay discovery until the actual shared canvas session has initialized.
    pub(super) fn context(
        &mut self,
        identity: MainIdentity,
        request: CanvasContextRequest,
        claim: MemoryClaim,
        mut session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        #[cfg(feature = "k1-native-qualification")]
        if !session.as_deref().is_some_and(Session::initialized) {
            sugarsubstitute_desktop::qualification_trace::progress(
                sugarsubstitute_desktop::qualification_trace::Progress::CanvasContextWaitingSession,
                identity,
            );
        }
        #[cfg(feature = "k1-native-qualification")]
        if !self.available(identity) {
            sugarsubstitute_desktop::qualification_trace::progress(
                sugarsubstitute_desktop::qualification_trace::Progress::CanvasContextWaitingBackend,
                identity,
            );
        }
        if let Some(previous) = self.binding.as_ref().map(|binding| binding.identity)
            && previous != identity
        {
            self.retire_navigation(previous, session.as_deref_mut())?;
        }
        let slot = self
            .contexts
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("Canvas context queue is full")?;
        *slot = Some(Context { identity, request, claim });
        if let Some(session) = session.filter(|session| session.initialized())
            && self.available(identity)
        {
            self.flush_contexts(session)?;
        }
        Ok(())
    }

    /// Accept only one ordered operation at a time from this exact attachment.
    pub(super) fn receive(
        &mut self,
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        allocation: Option<WebViewAllocation>,
        claim: MemoryClaim,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        let request = match message {
            CanvasSurfaceMessage::Discovery(request) => {
                return self.discover(identity, &request, claim, session);
            }
            CanvasSurfaceMessage::Cancel { request_id, .. } => {
                return self.cancel(identity, request_id, claim, Some(session));
            }
            CanvasSurfaceMessage::Request(request) => request,
            CanvasSurfaceMessage::Presentation(request) => {
                return self.watch_presentation(identity, request, claim, session);
            }
        };
        if self.rejected_schema == Some(identity)
            && !matches!(
                request.operation(),
                CanvasSurfaceOperation::Release | CanvasSurfaceOperation::PreviewWithdraw(_)
            )
        {
            lifecycle::replied(&request, CanvasSurfaceReplyStatus::Unavailable);
            return self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::new(
                identity, &request, CanvasSurfaceReplyStatus::Unavailable,
                "Native surface schema 2 is required. Update SugarCubes and its native host together.", claim,
            )?));
        }
        if matches!(
            request.operation(),
            CanvasSurfaceOperation::PreviewBegin(_)
                | CanvasSurfaceOperation::PreviewChunk(_)
                | CanvasSurfaceOperation::PreviewEnd(_)
                | CanvasSurfaceOperation::PreviewWithdraw(_)
        ) {
            return self.receive_preview(identity, request, claim, session);
        }
        if self.restoring
            || (self.pending.is_some()
                && !self.admits_beside_deferred_selection(identity, &request)
                && !self.withdraw_for_release(identity, &request, session)?)
        {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        let operation = request.operation().clone();
        if !self.available(identity) {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        if matches!(operation, CanvasSurfaceOperation::Prepare) {
            lifecycle::admitted(&request);
            return self.prepare(identity, request, claim, session);
        }
        let Some(binding) = &mut self.binding else {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Stale, claim);
        };
        if binding.identity != identity
            || binding.address != *request.address()
            || request.revision() <= binding.revision
        {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Stale, claim);
        }
        binding.revision = request.revision();
        lifecycle::admitted(&request);
        self.apply_operation(identity, request, allocation, claim, operation, session)
    }

    fn offer_comfy(
        &mut self,
        identity: MainIdentity,
        target: ComfyViewTarget,
        compared: Option<ComfyViewTarget>,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        if let Some(source) = self.session_source.take() {
            return Self::offer_acquired(source.single()?, session);
        }
        let backend = self
            .backend
            .filter(|backend| backend.identity == identity)
            .ok_or("Canvas backend grant disappeared")?;
        let owner = super::comfy_source::ComfySource::new(backend, target, compared)?;
        Self::offer_acquired(Arc::new(owner), session)
    }

    fn offer_comfy_overview(
        &mut self,
        identity: MainIdentity,
        selection: crate::native_canvas::session::OverviewSelection<ComfyViewTarget>,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        if let Some(source) = self.session_source.take() {
            return Self::offer_acquired(source.overview(selection)?, session);
        }
        let backend = self
            .backend
            .filter(|backend| backend.identity == identity)
            .ok_or("Canvas backend grant disappeared")?;
        let owner = super::comfy_source::ComfySource::overview(backend, selection)?;
        Self::offer_acquired(Arc::new(owner), session)
    }

    /// Offer an inert host acquisition adapter as the session's next source selection.
    fn offer_acquired(
        owner: Arc<dyn crate::native_canvas::session::acquisition::SourceAcquirer>,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        session.offer_acquirer(&mut Some(owner), CacheReuse::Compatible)
    }

    /// Answer waiting context reads with the capabilities and the input document the Input
    /// route shows, if any.
    fn flush_contexts(&mut self, session: &Session) -> Result<(), Box<dyn Error>> {
        let document = session
            .shown_input()
            .map(|shown| input_protocol::canvas_document(shown.document, shown.snapshot.extent()))
            .transpose()?;
        for index in 0..self.contexts.len() {
            if self.contexts[index].as_ref().is_none_or(|context| !self.available(context.identity))
            {
                continue;
            }
            let Some(context) = self.contexts[index].take() else { continue };
            let reply = CanvasNativeReply::new(
                context.identity,
                context.request.reply(native::ADVERTISED, document.as_ref())?,
                context.claim,
            )?;
            self.enqueue(DesktopStartupCommand::SendCanvasNative(reply))?;
            #[cfg(feature = "k1-native-qualification")]
            sugarsubstitute_desktop::qualification_trace::progress(
                sugarsubstitute_desktop::qualification_trace::Progress::CanvasContextReplyQueued,
                context.identity,
            );
        }
        Ok(())
    }

    fn reply(
        &mut self,
        identity: MainIdentity,
        request: &CanvasSurfaceRequest,
        status: CanvasSurfaceReplyStatus,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        let reply = SurfaceReply::new(identity, request, status, "", claim)?;
        lifecycle::replied(request, status);
        self.enqueue(DesktopStartupCommand::SendSurface(reply))
    }

    fn enqueue(&mut self, command: DesktopStartupCommand) -> Result<(), Box<dyn Error>> {
        let slot = self
            .replies
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("Canvas response queue is full")?;
        *slot = Some(command);
        Ok(())
    }

    /// Transfer the oldest reply with its native admission, keeping vacant slots at the tail.
    pub(super) fn take_command(&mut self) -> Option<DesktopStartupCommand> {
        if let Some(command) = self.replies[0].take() {
            self.replies.rotate_left(1);
            Some(command)
        } else {
            self.preview
                .notification
                .take()
                .or_else(|| self.preview.camera.take())
                .or_else(|| self.sources.notification.take())
                .or_else(|| self.editing.notification.take())
        }
    }

    /// Fence surface operations when the application starts closing.
    pub(super) fn close(&mut self, session: Option<&mut Session>) -> Result<(), Box<dyn Error>> {
        if let Some(session) = session {
            self.fence_previews(session)?;
        }
        self.closed = true;
        self.binding = None;
        self.retained = None;
        if let Some(pending) = self.pending.take() {
            lifecycle::retired(&pending.request, pending.kind);
        }
        self.contexts = std::array::from_fn(|_| None);
        self.replies = std::array::from_fn(|_| None);
        self.preview.close_watch();
        self.preview.close_correlations();
        self.workflow.close();
        self.sources.close();
        self.editing = editing::Editing::default();
        Ok(())
    }
}
