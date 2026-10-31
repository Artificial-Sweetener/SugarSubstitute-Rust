// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate the shared web surface with the current authorized canvas and backend.

mod attachment;
mod capabilities;
mod ingress;
mod navigation;
mod operations;
mod presentation;
mod preview;
#[cfg(test)]
#[path = "surface/replies_tests.rs"]
mod replies_tests;
mod viewport;

use super::*;
#[cfg(test)]
use std::path::{Component, Path};
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_comfy::output_media::ComfyViewTarget;
use sugarsubstitute_contracts::{
    canvas_context::{CanvasContextRequest, CanvasDisplayCapability},
    canvas_surface::{
        CanvasFileStorage, CanvasSurfaceAddress, CanvasSurfaceMessage, CanvasSurfaceOperation,
        CanvasSurfaceReplyStatus, CanvasSurfaceRequest,
    },
    execution::MemoryClaim,
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{
    CanvasContextReply, DesktopStartupCommand, SurfaceReply,
    canvas::{
        MainCanvasFrame, MainCanvasRetention,
        placement::{CanvasPlacement, WebViewAllocation, placement, preparation_viewport},
    },
};
use sugarsubstitute_image_io::CacheReuse;
use sugarsubstitute_image_io::ImageSource;

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
    selected: bool,
    selection_revision: u64,
    source_intent_revision: Option<u64>,
    presented_selection_revision: u64,
    presented_viewport: Option<sugarsubstitute_contracts::canvas_surface::CanvasViewportState>,
    committed: bool,
    presented: Option<MainCanvasRetention>,
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
    Viewport { intent: crate::native_canvas::session::CameraIntent, commanded: bool },
}

struct Pending {
    identity: MainIdentity,
    request: Box<CanvasSurfaceRequest>,
    claim: MemoryClaim,
    kind: PendingKind,
}

/// One native attachment and bounded replies for the current panel.
pub(super) struct NativeSurface {
    backend: Option<crate::startup::CanvasBackend>,
    #[cfg(test)]
    fixture_output: Option<PathBuf>,
    binding: Option<Binding>,
    retained: Option<Binding>,
    restoring: bool,
    pending: Option<Pending>,
    contexts: [Option<Context>; 4],
    replies: [Option<DesktopStartupCommand>; 8],
    preview: preview::Preview,
    closed: bool,
}

impl NativeSurface {
    /// Begin without a backend; only the authenticated startup owner can provide one.
    pub(super) fn new() -> Self {
        Self {
            backend: None,
            #[cfg(test)]
            fixture_output: None,
            binding: None,
            retained: None,
            restoring: false,
            pending: None,
            contexts: std::array::from_fn(|_| None),
            replies: std::array::from_fn(|_| None),
            preview: preview::Preview::default(),
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
    pub(super) fn backend(
        &mut self,
        backend: Option<crate::startup::CanvasBackend>,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.backend == backend {
            return Ok(());
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
        if session.as_deref().is_some_and(Session::initialized) && self.available(identity) {
            self.flush_contexts()?;
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
        if matches!(
            request.operation(),
            CanvasSurfaceOperation::PreviewBegin(_)
                | CanvasSurfaceOperation::PreviewChunk(_)
                | CanvasSurfaceOperation::PreviewEnd(_)
                | CanvasSurfaceOperation::PreviewWithdraw(_)
        ) {
            return self.receive_preview(identity, request, claim, session);
        }
        if self.pending.is_some() || self.restoring {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        let operation = request.operation().clone();
        if !self.available(identity) {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        if matches!(operation, CanvasSurfaceOperation::Prepare) {
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
        self.apply_operation(identity, request, allocation, claim, operation, session)
    }

    fn offer_comfy(
        &mut self,
        identity: MainIdentity,
        target: ComfyViewTarget,
        compared: Option<ComfyViewTarget>,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        let backend = self
            .backend
            .filter(|backend| backend.identity == identity)
            .ok_or("Canvas backend grant disappeared")?;
        let owner = super::comfy_source::ComfySource::new(backend, target, compared)?;
        let mut source: Option<
            Arc<dyn crate::native_canvas::session::acquisition::SourceAcquirer>,
        > = Some(Arc::new(owner));
        session.offer_acquirer(&mut source, CacheReuse::Compatible)
    }

    fn flush_contexts(&mut self) -> Result<(), Box<dyn Error>> {
        for index in 0..self.contexts.len() {
            if self.contexts[index].as_ref().is_none_or(|context| !self.available(context.identity))
            {
                continue;
            }
            let Some(context) = self.contexts[index].take() else { continue };
            let reply = CanvasContextReply::new(
                context.identity,
                &context.request,
                CanvasDisplayCapability::Available,
                context.claim,
            )?;
            self.enqueue(DesktopStartupCommand::SendCanvasContext(reply))?;
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
            self.preview.notification.take()
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
        self.pending = None;
        self.contexts = std::array::from_fn(|_| None);
        self.replies = std::array::from_fn(|_| None);
        self.preview.close_watch();
        self.preview.close_correlations();
        Ok(())
    }
}
