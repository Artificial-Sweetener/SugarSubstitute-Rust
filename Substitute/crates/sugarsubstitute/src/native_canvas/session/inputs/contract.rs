// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Requests the input document store takes and the results it reports.
use super::*;
use std::fmt;
/// Identity of one input document, disjoint from every output document identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct InputDocumentId(pub(super) u64);

impl InputDocumentId {
    /// The document identity its snapshots carry.
    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    /// The document's protocol identity: `input-` and its index in lowercase hexadecimal
    /// without leading zeros. It is the one name every message family (workflow, context,
    /// source, geometry and editing) and the archive use, stable for the surface's lifetime
    /// and across restarts.
    pub(crate) fn protocol_id(self) -> String {
        format!("input-{:x}", self.0 & !IDENTITY_NAMESPACE)
    }

    /// The identity [`Self::protocol_id`] names; this does not check that it is synced.
    pub(crate) fn from_protocol_id(text: &str) -> Option<Self> {
        let digits = text.strip_prefix("input-")?;
        if digits.is_empty() || digits.starts_with('0') || digits.len() > 16 {
            return None;
        }
        let value = u64::from_str_radix(digits, 16).ok()?;
        (value & IDENTITY_NAMESPACE == 0).then_some(Self(value | IDENTITY_NAMESPACE))
    }

    /// The geometry revision of this document at `extent`: it changes exactly when the
    /// document's pixel geometry changes.
    pub(crate) fn geometry(self, extent: Extent) -> String {
        format!("{}-{}x{}", self.protocol_id(), extent.width(), extent.height())
    }
}

/// A layer's protocol identity: `layer-` and its decimal identity, unique within its document.
/// Every message family names layers, mask layers included, by it.
pub(crate) fn layer_protocol_id(layer: LayerId) -> String {
    format!("layer-{}", layer.get())
}

/// The layer [`layer_protocol_id`] names; this does not check that the layer exists.
pub(crate) fn layer_from_protocol_id(text: &str) -> Option<LayerId> {
    let digits = text.strip_prefix("layer-")?;
    if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    LayerId::new(digits.parse().ok()?).ok()
}

/// The input document the Input route shows, with its content and editing target.
pub(crate) struct ShownInput {
    /// Its identity.
    pub(crate) document: InputDocumentId,
    /// Its current content.
    pub(crate) snapshot: DocumentSnapshot,
    /// The active mask the editing tools target, if any.
    pub(crate) target: Option<LayerId>,
}

/// What one synced document shows: its published content revision and, while a gesture stages
/// a preview on that revision, the preview's step. Watching reports both, so previews follow a
/// stroke while it is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShownRevision {
    /// The published content revision.
    pub(crate) revision: u64,
    /// The staged preview step shown on it, or 0 when none is staged.
    pub(crate) staged: u64,
}

impl ShownRevision {
    /// A published `revision` with no staged step.
    pub(crate) const fn published(revision: u64) -> Self {
        Self { revision, staged: 0 }
    }
}

/// A ComfyUI file, named by its host reference text, and the inert one-use adapter that
/// acquires it. The canvas compares references only; the adapter is used at most once.
#[derive(Clone)]
pub(crate) struct ComfyFile {
    /// Host reference text identifying the file; a change means a different file.
    pub(crate) reference: Arc<str>,
    /// Adapter that fetches the file on an admitted worker.
    pub(crate) acquirer: Arc<dyn acquisition::SourceAcquirer>,
}

/// What a surface's locked base layer shows.
#[derive(Clone)]
pub(crate) enum SurfaceBase {
    /// A ComfyUI input image; its pixel size is the document's.
    File(ComfyFile),
    /// An RGB(24,24,24) backing of this size, applied only when the surface is created.
    Blank(Extent),
}

/// The loader channel a mask file is read through, as ComfyUI's `LoadImageMask` reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaskChannel {
    /// The red sample.
    Red,
    /// The green sample.
    Green,
    /// The blue sample.
    Blue,
    /// One minus the alpha sample.
    Alpha,
}

/// One mask a surface provides.
#[derive(Clone)]
pub(crate) struct MaskSpec {
    /// Stable mask key within its surface.
    pub(crate) key: Arc<str>,
    /// Layer name.
    pub(crate) label: Arc<str>,
    /// Synced overlay appearance; applied only when it differs from the last synced value.
    pub(crate) appearance: MaskAppearance,
    /// Channel the mask's file is read through.
    pub(crate) channel: MaskChannel,
    /// File loaded when the mask is created or this reference changes; `None` never clears.
    pub(crate) initial: Option<ComfyFile>,
}

/// One surface of a workflow: a base and its masks.
#[derive(Clone)]
pub(crate) struct SurfaceSpec {
    /// Stable surface key within its workflow.
    pub(crate) key: Arc<str>,
    /// Base layer name.
    pub(crate) label: Arc<str>,
    /// Base layer content.
    pub(crate) base: SurfaceBase,
    /// Masks, bottom to top above the base.
    pub(crate) masks: Vec<MaskSpec>,
}

/// One mask's requested overlay appearance.
#[derive(Clone)]
pub(crate) struct AppearanceChange {
    /// Surface key.
    pub(crate) surface: Arc<str>,
    /// Mask key.
    pub(crate) mask: Arc<str>,
    /// New tint and opacity.
    pub(crate) appearance: MaskAppearance,
}
/// Fixed point of a canvas resize, in the nine positions of the resolution dialog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResizeAnchor {
    /// Keep the top-left corner.
    TopLeft,
    /// Keep the top edge's center.
    Top,
    /// Keep the top-right corner.
    TopRight,
    /// Keep the left edge's center.
    Left,
    /// Keep the center.
    Center,
    /// Keep the right edge's center.
    Right,
    /// Keep the bottom-left corner.
    BottomLeft,
    /// Keep the bottom edge's center.
    Bottom,
    /// Keep the bottom-right corner.
    BottomRight,
}

impl ResizeAnchor {
    /// Horizontal and vertical position as halves: 0 start, 1 center, 2 end.
    pub(super) const fn halves(self) -> [i64; 2] {
        match self {
            Self::TopLeft => [0, 0],
            Self::Top => [1, 0],
            Self::TopRight => [2, 0],
            Self::Left => [0, 1],
            Self::Center => [1, 1],
            Self::Right => [2, 1],
            Self::BottomLeft => [0, 2],
            Self::Bottom => [1, 2],
            Self::BottomRight => [2, 2],
        }
    }
}

/// How a blank-based surface changes size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResizeMode {
    /// Change the canvas only; masks keep their pixels, placed by the anchor.
    Canvas(ResizeAnchor),
    /// Scale the canvas and resample every mask.
    Scale(CoverageResampling),
}

/// Completion handle of one sync or resize; its reply arrives through
/// [`Session::take_input_replies`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InputTicket(pub(super) u64);

/// A worker or acquisition failure, with its original cause.
#[derive(Clone, Debug)]
pub(crate) struct InputFailure(Arc<dyn Error + Send + Sync>);

impl InputFailure {
    pub(super) fn new(cause: impl Error + Send + Sync + 'static) -> Self {
        Self(Arc::new(cause))
    }
    pub(super) fn message(message: &'static str) -> Self {
        Self::new(std::io::Error::other(message))
    }
}

impl fmt::Display for InputFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl Error for InputFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// What became of a mask's initial file.
#[derive(Clone, Debug)]
pub(crate) enum InitialOutcome {
    /// The file's channel now fills the mask.
    Loaded,
    /// The file's size differs from the document's; the mask was left as it was.
    Mismatch {
        /// Document size.
        expected: Extent,
        /// File size.
        actual: Extent,
    },
    /// The file could not be acquired or decoded; a later sync retries it.
    Failed(InputFailure),
}

/// One mask of a synced surface.
#[derive(Clone, Debug)]
pub(crate) struct SyncedMask {
    /// Mask key.
    pub(crate) key: Arc<str>,
    /// Its coverage layer.
    pub(crate) layer: LayerId,
    /// Outcome of its latest initial file, when one was requested.
    pub(crate) initial: Option<InitialOutcome>,
}

/// A loaded surface's document as a reply reports it.
#[derive(Clone, Debug)]
pub(crate) struct SyncedSurface {
    /// Surface key.
    pub(crate) key: Arc<str>,
    /// Document identity.
    pub(crate) document: InputDocumentId,

    /// Changes exactly when the document's pixel geometry changes.
    pub(crate) geometry: String,
    /// Document pixel size.
    pub(crate) extent: Extent,
    /// Masks in sync order.
    pub(crate) masks: Vec<SyncedMask>,
}

/// One surface of a reply.
#[derive(Clone, Debug)]
pub(crate) enum SurfaceOutcome {
    /// The surface's document is loaded and current.
    Synced(SyncedSurface),
    /// The surface's base could not be loaded, a resize could not apply, or a later sync
    /// retired the surface.
    Failed {
        /// Surface key.
        key: Arc<str>,
        /// Cause.
        failure: InputFailure,
    },
}

/// The settled state of every surface a sync or resize named.
#[derive(Clone, Debug)]
pub(crate) struct InputReply {
    /// The request this answers.
    pub(crate) ticket: InputTicket,
    /// Workflow identity.
    pub(crate) workflow: Arc<str>,
    /// Surfaces in request order.
    pub(crate) surfaces: Vec<SurfaceOutcome>,
}

/// A synced document as a capture reads it.
#[derive(Clone)]
pub(crate) enum InputSnapshot {
    /// The document's current content, at the expected revision when one was given.
    Current(DocumentSnapshot),
    /// The document changed since the expected revision, or workflow work is about to change it.
    Stale,
    /// No synced document has this identity, or it is not loaded yet.
    Unavailable,
}

/// A request the store cannot take; nothing changed.
#[derive(Debug)]
pub(crate) enum InputRejection {
    /// The request breaks a limit or names a key twice.
    Invalid(&'static str),
    /// The workflow, surface or mask is not synced.
    Unknown,
    /// The surface's document is not loaded, or the canvas is closing.
    Unavailable(&'static str),
}

impl fmt::Display for InputRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Unavailable(message) => formatter.write_str(message),
            Self::Unknown => formatter.write_str("The input surface is not synced"),
        }
    }
}

impl Error for InputRejection {}
