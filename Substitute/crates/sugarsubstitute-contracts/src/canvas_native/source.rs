// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode `source.request` and encode its context, capture, watch and change messages.

use super::{
    CanvasDocument, CanvasDocumentId, CanvasFailure, CanvasLayerId, CanvasLayerName,
    CanvasNativeEnvelope, CanvasNativeError, CanvasNativeNotificationKind,
    CanvasNativeNotificationPayload, CanvasNativeReplyPayload, CanvasPixelRaster, CanvasRevision,
    MAX_CANVAS_SOURCE_CHANGED_BYTES, MAX_CANVAS_SOURCE_REPLY_BYTES,
    MAX_CANVAS_SOURCE_REQUEST_BYTES, ReplyEnvelope, admit, parse, wire,
};
use crate::panel::PanelSession;

/// What a capture reads from a document.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CanvasSourceTarget {
    /// The selection, captured as `coverage8`.
    Selection,
    /// The current layer.
    CurrentLayer,
    /// One named layer; a mask layer captures as `rgba8` in the coverage image encoding.
    Layer(CanvasLayerId),
    /// The visible image layers composed; mask overlays are never part of it.
    Composition,
    /// The composition from the named layer downward.
    CompositionFromLayer(CanvasLayerId),
}

impl CanvasSourceTarget {
    /// Return the target's kind without its layer identity.
    pub const fn kind(&self) -> CanvasSourceTargetKind {
        match self {
            Self::Selection => CanvasSourceTargetKind::Selection,
            Self::CurrentLayer => CanvasSourceTargetKind::CurrentLayer,
            Self::Layer(_) => CanvasSourceTargetKind::Layer,
            Self::Composition => CanvasSourceTargetKind::Composition,
            Self::CompositionFromLayer(_) => CanvasSourceTargetKind::CompositionFromLayer,
        }
    }

    fn from_wire(value: wire::CanvasSourceTarget) -> Result<Self, CanvasNativeError> {
        Ok(match value {
            wire::CanvasSourceTarget::Selection {} => Self::Selection,
            wire::CanvasSourceTarget::CurrentLayer {} => Self::CurrentLayer,
            wire::CanvasSourceTarget::Layer { layer_id } => {
                Self::Layer(CanvasLayerId::new(layer_id)?)
            }
            wire::CanvasSourceTarget::Composition {} => Self::Composition,
            wire::CanvasSourceTarget::CompositionFromLayer { layer_id } => {
                Self::CompositionFromLayer(CanvasLayerId::new(layer_id)?)
            }
        })
    }

    fn to_wire(&self) -> wire::CanvasSourceTarget {
        match self {
            Self::Selection => wire::CanvasSourceTarget::Selection {},
            Self::CurrentLayer => wire::CanvasSourceTarget::CurrentLayer {},
            Self::Layer(id) => wire::CanvasSourceTarget::Layer { layer_id: id.as_str().into() },
            Self::Composition => wire::CanvasSourceTarget::Composition {},
            Self::CompositionFromLayer(id) => {
                wire::CanvasSourceTarget::CompositionFromLayer { layer_id: id.as_str().into() }
            }
        }
    }
}

/// The capture target kinds a source context advertises.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasSourceTargetKind {
    /// [`CanvasSourceTarget::Selection`].
    Selection,
    /// [`CanvasSourceTarget::CurrentLayer`].
    CurrentLayer,
    /// [`CanvasSourceTarget::Layer`].
    Layer,
    /// [`CanvasSourceTarget::Composition`].
    Composition,
    /// [`CanvasSourceTarget::CompositionFromLayer`].
    CompositionFromLayer,
}

impl CanvasSourceTargetKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::CurrentLayer => "current-layer",
            Self::Layer => "layer",
            Self::Composition => "composition",
            Self::CompositionFromLayer => "composition-from-layer",
        }
    }
}

/// The resolution a capture returns.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasCaptureSize {
    /// Full document resolution.
    Full,
    /// Scaled so the longer side is at most this many pixels, `1..=1024`; never enlarged.
    Preview(u16),
}

/// Capture the listed targets of one document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceCapture {
    /// The document to read; any synced input document qualifies, not only the active one.
    pub document: CanvasDocument,
    /// Up to 64 targets, answered in order.
    pub targets: Vec<CanvasSourceTarget>,
    /// The requested resolution.
    pub size: CanvasCaptureSize,
}

/// One `source.request` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanvasSourceCommand {
    /// Describe the active document's layers and capture targets.
    Context,
    /// Capture pixels; a changed or retired document answers `stale` or `unavailable`.
    Capture(CanvasSourceCapture),
    /// Start or stop `source.changed` notifications.
    Watch {
        /// Whether notifications are wanted.
        enabled: bool,
    },
}

/// A validated `source.request` from one panel session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceRequest {
    envelope: CanvasNativeEnvelope,
    command: CanvasSourceCommand,
}

/// One layer a source context lists; mask layers appear like any other layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceLayer {
    /// Host-assigned layer identity.
    pub id: CanvasLayerId,
    /// User-visible name.
    pub name: CanvasLayerName,
    /// Containing group, if any.
    pub parent: Option<CanvasLayerId>,
    /// Nesting depth, `0..=128`; encoding rejects deeper values.
    pub depth: u8,
    /// Whether the layer is visible.
    pub visible: bool,
}

/// The answer to [`CanvasSourceCommand::Context`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceContext {
    /// Whether the host captures sources at all.
    pub supported: bool,
    /// The active document, if one is shown.
    pub document: Option<CanvasDocument>,
    /// Opaque content revision; `source.changed` reports later ones.
    pub revision: CanvasRevision,
    /// Distinct capture target kinds offered, at most five.
    pub targets: Vec<CanvasSourceTargetKind>,
    /// Up to 4096 layers, top first.
    pub layers: Vec<CanvasSourceLayer>,
    /// The current layer, if any.
    pub current_layer: Option<CanvasLayerId>,
    /// Whether `source.changed` notifications are available.
    pub observes_changes: bool,
}

/// One captured target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceSnapshot {
    /// The target this snapshot answers.
    pub target: CanvasSourceTarget,
    /// The document as captured.
    pub document: CanvasDocument,
    /// The content revision the pixels show.
    pub revision: CanvasRevision,
    /// The captured pixels.
    pub pixels: CanvasPixelRaster,
}

/// The `source.changed` notification: a document's content revision changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSourceChange {
    /// The changed document, or none when no document is shown any more.
    pub document: Option<CanvasDocumentId>,
    /// The new opaque content revision.
    pub revision: CanvasRevision,
}

impl CanvasSourceRequest {
    /// Decode the exact shared `source.request`.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_SOURCE_REQUEST_BYTES)?;
        let request: wire::CanvasSourceRequest = parse(bytes)?;
        if !request.valid() {
            return Err(CanvasNativeError::Invalid);
        }
        let command = match request.command {
            wire::CanvasSourceCommand::Context {} => CanvasSourceCommand::Context,
            wire::CanvasSourceCommand::Capture { document, targets, maximum_size } => {
                CanvasSourceCommand::Capture(CanvasSourceCapture {
                    document: CanvasDocument::from_wire(document)?,
                    targets: targets
                        .into_iter()
                        .map(CanvasSourceTarget::from_wire)
                        .collect::<Result<_, _>>()?,
                    size: match u16::try_from(maximum_size) {
                        Ok(0) => CanvasCaptureSize::Full,
                        Ok(edge @ 1..=1024) => CanvasCaptureSize::Preview(edge),
                        _ => return Err(CanvasNativeError::Invalid),
                    },
                })
            }
            wire::CanvasSourceCommand::Watch { enabled } => CanvasSourceCommand::Watch { enabled },
        };
        Ok(Self {
            envelope: CanvasNativeEnvelope::new(request.session, request.request_id)?,
            command,
        })
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        &self.envelope
    }

    /// Borrow the decoded command.
    pub const fn command(&self) -> &CanvasSourceCommand {
        &self.command
    }

    /// Encode the answer to a context command.
    ///
    /// # Errors
    /// Rejects more than five target kinds, more than 4096 layers or a depth above 128, and
    /// preserves serialization failures and the reply bound.
    pub fn context_reply(
        &self,
        context: &CanvasSourceContext,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasSourceResult::Context {
            supported: context.supported,
            document: context.document.as_ref().map(CanvasDocument::to_wire),
            revision: context.revision.as_str().into(),
            targets: context.targets.iter().map(|kind| kind.as_str().into()).collect(),
            layers: context
                .layers
                .iter()
                .map(|layer| wire::CanvasSourceLayer {
                    id: layer.id.as_str().into(),
                    name: layer.name.as_str().into(),
                    parent_id: layer.parent.as_ref().map(|id| id.as_str().into()),
                    depth: layer.depth.into(),
                    visible: layer.visible,
                })
                .collect(),
            current_layer_id: context.current_layer.as_ref().map(|id| id.as_str().into()),
            observes_changes: context.observes_changes,
        })
    }

    /// Encode captured snapshots, one per requested target.
    ///
    /// Encoding allocates each raster's base64 text and the complete JSON; callers hold
    /// storage for both. A reply over 16 KiB reaches the page through the bulk channel.
    /// # Errors
    /// Rejects more than 64 snapshots and replies above [`MAX_CANVAS_SOURCE_REPLY_BYTES`].
    pub fn captured_reply(
        &self,
        snapshots: &[CanvasSourceSnapshot],
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasSourceResult::Captured {
            snapshots: snapshots
                .iter()
                .map(|snapshot| wire::CanvasSourceSnapshot {
                    target: snapshot.target.to_wire(),
                    document: snapshot.document.to_wire(),
                    revision: snapshot.revision.as_str().into(),
                    pixels: snapshot.pixels.to_wire(),
                })
                .collect(),
        })
    }

    /// Encode the answer to a watch command.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn watching_reply(
        &self,
        supported: bool,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasSourceResult::Watching { supported })
    }

    /// Encode a typed failure for any source command.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn failure_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let wire::CanvasFailure { status, message } = failure.to_wire();
        self.reply(wire::CanvasSourceResult::Failure { status, message })
    }

    fn reply(
        &self,
        result: wire::CanvasSourceResult,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session, request_id } =
            ReplyEnvelope::of(&self.envelope);
        let reply = wire::CanvasSourceReply {
            protocol,
            version,
            session,
            request_id,
            r#type: "source.reply".into(),
            result,
        };
        self.envelope.payload(&reply, reply.valid(), MAX_CANVAS_SOURCE_REPLY_BYTES)
    }
}

impl CanvasSourceChange {
    /// Encode `source.changed` for one panel session.
    ///
    /// # Errors
    /// Preserves serialization failures and the notification bound.
    pub fn encode(
        &self,
        session: &PanelSession,
    ) -> Result<CanvasNativeNotificationPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session: name, request_id } =
            ReplyEnvelope::notification(session);
        let notification = wire::CanvasSourceChanged {
            protocol,
            version,
            session: name,
            request_id,
            r#type: "source.changed".into(),
            change: wire::CanvasSourceChange {
                document_id: self.document.as_ref().map(|id| id.as_str().into()),
                revision: self.revision.as_str().into(),
            },
        };
        CanvasNativeNotificationPayload::new(
            session,
            CanvasNativeNotificationKind::SourceChanged,
            &notification,
            notification.valid(),
            MAX_CANVAS_SOURCE_CHANGED_BYTES,
        )
    }
}
