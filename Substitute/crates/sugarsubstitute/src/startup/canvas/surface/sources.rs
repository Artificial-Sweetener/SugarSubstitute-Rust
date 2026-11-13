// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve `source.request` from the session's input documents.
//!
//! The context describes the input document the Input route shows. Captures read any synced
//! input document, current or parked, and run as session jobs that encode their reply on the
//! worker; the reply reaches the page through the bulk channel when it is large. Captures read
//! what the canvas shows, including a stroke while it is drawn, so card previews stay live.
//! Watching reports `source.changed` whenever what a synced document shows changes, and when
//! the Input route shows a document again or stops showing one, one notification at a time, so
//! a newer change of one document supersedes an undelivered older one.
use super::input_protocol::{
    canvas_document, document_id, document_of, failure, layer_key, layer_of,
};
use super::*;
use crate::native_canvas::session::inputs::{
    CaptureEncoder, CapturePixels, CaptureRejection, CaptureTarget, CaptureTicket, EncodedCapture,
    InputCapture, InputDocumentId, InputFailure, ShownRevision, capture_extent,
};
use std::collections::VecDeque;
use sugarsubstitute_contracts::canvas_native::{
    CanvasCaptureSize, CanvasFailureStatus, CanvasLayerName, CanvasNativeError, CanvasPixelFormat,
    CanvasPixelRaster, CanvasRevision, CanvasSourceCapture, CanvasSourceChange,
    CanvasSourceCommand, CanvasSourceContext, CanvasSourceLayer, CanvasSourceRequest,
    CanvasSourceSnapshot, CanvasSourceTarget, CanvasSourceTargetKind,
    MAX_CANVAS_SOURCE_REPLY_BYTES,
};
use sugarsubstitute_desktop::CanvasNativeNotification;
use sugarsubstitute_desktop::CanvasNativeRequestStorage;
use sugarsubstitute_image_ops::Extent;

/// Captures that may wait for their jobs.
const MAX_HELD: usize = 8;
/// Reply bytes beyond base64 pixels: envelope and each snapshot's target and document fields.
const SNAPSHOT_OVERHEAD_BYTES: u64 = 2048;
/// The target kinds every input document offers.
const TARGETS: [CanvasSourceTargetKind; 5] = [
    CanvasSourceTargetKind::Selection,
    CanvasSourceTargetKind::CurrentLayer,
    CanvasSourceTargetKind::Layer,
    CanvasSourceTargetKind::Composition,
    CanvasSourceTargetKind::CompositionFromLayer,
];

struct Held {
    identity: MainIdentity,
    request: CanvasSourceRequest,
    claim: MemoryClaim,
    _storage: CanvasNativeRequestStorage,
    ticket: CaptureTicket,
    outcome: Option<Result<EncodedCapture, InputFailure>>,
}

/// One navigation's `source.changed` subscription.
struct Watch {
    identity: MainIdentity,
    /// Last reported revision of every synced document, and the shown document.
    revisions: Vec<(InputDocumentId, ShownRevision)>,
    shown: Option<InputDocumentId>,
    /// Changes not yet notified, oldest first; `None` reports that no document is shown.
    changes: VecDeque<(Option<InputDocumentId>, ShownRevision)>,
}

/// Held captures and the change subscription.
#[derive(Default)]
pub(super) struct Sources {
    held: VecDeque<Held>,
    watch: Option<Watch>,
    /// The undelivered `source.changed`; it owns no reply slot.
    pub(super) notification: Option<DesktopStartupCommand>,
}

impl Sources {
    /// Drop a retired navigation's captures and subscription.
    pub(super) fn retire(&mut self, identity: MainIdentity) {
        self.held.retain(|held| held.identity != identity);
        if self.watch.as_ref().is_some_and(|watch| watch.identity == identity) {
            self.watch = None;
            self.notification = None;
        }
    }

    /// Drop every held capture and the subscription at close.
    pub(super) fn close(&mut self) {
        self.held.clear();
        self.watch = None;
        self.notification = None;
    }
}

/// The opaque revision of what one document shows; a staged stroke step adds its own suffix.
fn revision(
    document: InputDocumentId,
    shown: ShownRevision,
) -> Result<CanvasRevision, CanvasNativeError> {
    let ShownRevision { revision, staged } = shown;
    CanvasRevision::new(match staged {
        0 => format!("{}@{revision}", document.protocol_id()),
        _ => format!("{}@{revision}+{staged}", document.protocol_id()),
    })
}

impl NativeSurface {
    /// Serve `source.request`: the shown document's layers, captures and change watching.
    pub(super) fn receive_source(
        &mut self,
        identity: MainIdentity,
        request: CanvasSourceRequest,
        claim: MemoryClaim,
        storage: CanvasNativeRequestStorage,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(session) = session.filter(|session| session.initialized()) else {
            let failure = failure(CanvasFailureStatus::Unavailable, "The canvas is not ready");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        };
        match request.command() {
            CanvasSourceCommand::Context => {
                let context = source_context(session)?;
                self.answer(identity, request.context_reply(&context)?, claim)
            }
            CanvasSourceCommand::Watch { enabled } => {
                if *enabled {
                    let shown = session.shown_input().map(|shown| shown.document);
                    let revisions = session.input_revisions();
                    let changes = VecDeque::new();
                    self.sources.watch = Some(Watch { identity, revisions, shown, changes });
                } else if self
                    .sources
                    .watch
                    .as_ref()
                    .is_some_and(|watch| watch.identity == identity)
                {
                    self.sources.watch = None;
                    self.sources.notification = None;
                }
                self.answer(identity, request.watching_reply(true)?, claim)
            }
            CanvasSourceCommand::Capture(capture) => {
                let capture = capture.clone();
                self.capture(identity, request, claim, storage, &capture, session)
            }
        }
    }

    fn capture(
        &mut self,
        identity: MainIdentity,
        request: CanvasSourceRequest,
        claim: MemoryClaim,
        storage: CanvasNativeRequestStorage,
        capture: &CanvasSourceCapture,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let unavailable = |message| failure(CanvasFailureStatus::Unavailable, message);
        let Some(document) = document_of(&capture.document.id) else {
            let failure = unavailable("The document is not an input document");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        };
        let targets = capture.targets.iter().map(target).collect::<Option<Vec<_>>>();
        let Some(targets) = targets else {
            let failure = unavailable("A captured layer is not a canvas layer");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        };
        if self.sources.held.len() >= MAX_HELD {
            let failure = unavailable("Too many captures wait");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        }
        let size = capture.document.size;
        let edge = match capture.size {
            CanvasCaptureSize::Full => None,
            CanvasCaptureSize::Preview(edge) => Some(u32::from(edge)),
        };
        let extent = capture_extent(Extent::new(size.width(), size.height())?, edge)?;
        let pixels = extent.pixels();
        let encoded = targets
            .iter()
            .map(|target| match target {
                CaptureTarget::Selection => pixels,
                _ => pixels.saturating_mul(4),
            })
            .map(|bytes| {
                bytes.div_ceil(3).saturating_mul(4).saturating_add(SNAPSHOT_OVERHEAD_BYTES)
            })
            .fold(0_u64, u64::saturating_add);
        let largest = targets.iter().any(|target| *target != CaptureTarget::Selection);
        let raster = pixels.saturating_mul(if largest { 4 } else { 1 });
        if encoded > MAX_CANVAS_SOURCE_REPLY_BYTES as u64
            || raster > CanvasPixelRaster::MAX_BYTES as u64
        {
            let failure =
                failure(CanvasFailureStatus::Error, "The capture exceeds its reply bound");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        }
        let encode = encoder(request.clone(), capture.targets.clone());
        let geometry = capture.document.geometry_revision.as_str();
        match session.capture_input(document, geometry, &targets, extent, encode) {
            Ok(ticket) => {
                let outcome = None;
                let _storage = storage;
                let held = Held { identity, request, claim, _storage, ticket, outcome };
                self.sources.held.push_back(held);
                Ok(())
            }
            Err(rejection) => {
                let failure = match rejection {
                    CaptureRejection::Stale => {
                        failure(CanvasFailureStatus::Stale, "The document changed")
                    }
                    CaptureRejection::Unavailable => {
                        unavailable("The document or layer is not available")
                    }
                    CaptureRejection::Busy => unavailable("The canvas cannot capture now"),
                };
                self.answer(identity, request.failure_reply(&failure)?, claim)
            }
        }
    }

    /// Answer finished captures while reply slots are free, then notify source changes.
    pub(super) fn poll_sources(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        for outcome in session.take_input_captures() {
            if let Some(held) =
                self.sources.held.iter_mut().find(|held| held.ticket == outcome.ticket)
            {
                held.outcome = Some(outcome.result);
            }
        }
        while self.free_replies() > 0 {
            let Some(index) = self.sources.held.iter().position(|held| held.outcome.is_some())
            else {
                break;
            };
            let Some(held) = self.sources.held.remove(index) else { break };
            match held.outcome.ok_or("Held capture outcome disappeared")? {
                Ok(EncodedCapture { payload, storage }) => {
                    let reply = CanvasNativeReply::with_storage(
                        held.identity,
                        payload,
                        held.claim,
                        storage,
                    )?;
                    self.enqueue(DesktopStartupCommand::SendCanvasNative(reply))?;
                }
                Err(cause) => {
                    tracing::warn!(operation = "canvas_source_capture", request_id = held.request.envelope().request_id(), cause = %cause, "Input document capture failed");
                    let failure = failure(CanvasFailureStatus::Error, &cause.to_string());
                    self.answer(held.identity, held.request.failure_reply(&failure)?, held.claim)?;
                }
            }
        }
        self.notify_sources(session)
    }

    /// Queue the oldest unreported change once the previous notification was delivered.
    fn notify_sources(&mut self, session: &Session) -> Result<(), Box<dyn Error>> {
        let Some(watch) = &mut self.sources.watch else { return Ok(()) };
        let current = session.input_revisions();
        let shown = session.shown_input().map(|shown| shown.document);
        for (document, value) in &current {
            let known = watch.revisions.iter().find(|(known, _)| known == document);
            // A document the route shows again changes the context sources resolve against,
            // so sources that could not resolve without it are read again.
            let reshown = shown == Some(*document) && watch.shown != shown;
            if reshown || known.is_none_or(|(_, known)| known != value) {
                watch.changes.retain(|(change, _)| *change != Some(*document));
                watch.changes.push_back((Some(*document), *value));
            }
        }
        watch.revisions = current;
        if shown.is_none() && watch.shown.is_some() {
            watch.changes.retain(|(change, _)| change.is_some());
            watch.changes.push_back((None, ShownRevision::published(0)));
        }
        watch.shown = shown;
        if self.sources.notification.is_some() {
            return Ok(());
        }
        let Some((document, value)) = watch.changes.front().copied() else { return Ok(()) };
        let Some(memory) = session.preview_resident() else { return Ok(()) };
        let Ok(storage) = memory.claim(CanvasNativeNotification::STORAGE_BYTES) else {
            return Ok(());
        };
        let change = CanvasSourceChange {
            document: document.map(document_id).transpose()?,
            revision: match document {
                Some(document) => revision(document, value)?,
                None => CanvasRevision::new("none")?,
            },
        };
        let panel = sugarsubstitute_contracts::panel::PanelSession::for_main(watch.identity)?;
        let notification =
            CanvasNativeNotification::new(watch.identity, change.encode(&panel)?, storage)?;
        watch.changes.pop_front();
        self.sources.notification =
            Some(DesktopStartupCommand::SendCanvasNotification(notification));
        Ok(())
    }

    /// Answer a held capture with `cancelled` and stop its job. Returns whether one matched.
    pub(super) fn cancel_capture(
        &mut self,
        identity: MainIdentity,
        request_id: u64,
        session: Option<&mut Session>,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(index) = self.sources.held.iter().position(|held| {
            held.identity == identity && held.request.envelope().request_id() == request_id
        }) else {
            return Ok(false);
        };
        let held = self.sources.held.remove(index).ok_or("Held capture disappeared")?;
        if let Some(session) = session {
            session.cancel_input_capture(held.ticket);
        }
        self.answer(identity, held.request.envelope().cancelled_reply()?, held.claim)?;
        Ok(true)
    }
}

/// The context of the input document the Input route shows; with none shown, no targets.
fn source_context(session: &Session) -> Result<CanvasSourceContext, Box<dyn Error>> {
    let Some(shown) = session.shown_input() else {
        return Ok(CanvasSourceContext {
            supported: true,
            document: None,
            revision: CanvasRevision::new("none")?,
            targets: Vec::new(),
            layers: Vec::new(),
            current_layer: None,
            observes_changes: true,
        });
    };
    let snapshot = &shown.snapshot;
    let layers = snapshot
        .layers()
        .iter()
        .rev()
        .map(|layer| {
            Ok(CanvasSourceLayer {
                id: layer_key(layer.id())?,
                name: CanvasLayerName::new(layer.name())?,
                parent: None,
                depth: 0,
                visible: layer.visible(),
            })
        })
        .collect::<Result<_, Box<dyn Error>>>()?;
    Ok(CanvasSourceContext {
        supported: true,
        document: Some(canvas_document(shown.document, snapshot.extent())?),
        revision: revision(shown.document, ShownRevision::published(snapshot.revision()))?,
        targets: TARGETS.to_vec(),
        layers,
        current_layer: shown.target.map(layer_key).transpose()?,
        observes_changes: true,
    })
}

/// The session target a protocol target names; `None` for a layer identity the canvas never
/// assigned.
fn target(target: &CanvasSourceTarget) -> Option<CaptureTarget> {
    Some(match target {
        CanvasSourceTarget::Selection => CaptureTarget::Selection,
        CanvasSourceTarget::CurrentLayer => CaptureTarget::CurrentLayer,
        CanvasSourceTarget::Layer(layer) => CaptureTarget::Layer(layer_of(layer)?),
        CanvasSourceTarget::Composition => CaptureTarget::Composition,
        CanvasSourceTarget::CompositionFromLayer(layer) => {
            CaptureTarget::CompositionFrom(layer_of(layer)?)
        }
    })
}

/// Encode a finished capture as `request`'s reply, one snapshot per target in order.
fn encoder(request: CanvasSourceRequest, targets: Vec<CanvasSourceTarget>) -> CaptureEncoder {
    Box::new(move |capture: InputCapture| {
        let document = canvas_document(capture.document, capture.extent)?;
        let revision = revision(capture.document, ShownRevision::published(capture.revision))?;
        let snapshots = targets
            .into_iter()
            .zip(capture.rasters)
            .map(|(target, raster)| {
                let format = match raster.pixels {
                    CapturePixels::Selection => CanvasPixelFormat::Coverage8,
                    CapturePixels::Image(_) | CapturePixels::Mask(_) => CanvasPixelFormat::Rgba8,
                };
                Ok(CanvasSourceSnapshot {
                    target,
                    document: document.clone(),
                    revision: revision.clone(),
                    pixels: CanvasPixelRaster::new(
                        raster.extent.width(),
                        raster.extent.height(),
                        format,
                        raster.bytes,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, CanvasNativeError>>()?;
        request.captured_reply(&snapshots)
    })
}
