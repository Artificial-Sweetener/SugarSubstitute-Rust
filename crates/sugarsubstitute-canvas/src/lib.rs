// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native canvas document and viewport state independent of windows and render devices.
mod archive;
mod comparison;
mod compile;
mod coverage;
mod cursor;
mod damage;
mod document;
mod error;
mod navigation;
mod overlay;
mod paint;
mod presentation;
mod projection;
mod tools;
mod viewport;
pub use archive::{
    ArchiveError, ArchiveIntegrity, ArchivedDocument, INPUT_ARCHIVE_VERSION, InputDocumentArchive,
    MAX_ARCHIVE_BYTES, MAX_ARCHIVED_DOCUMENTS, MAX_DOCUMENT_KEY_BYTES, MAX_HOST_METADATA_BYTES,
    MAX_SOURCE_REFERENCE_BYTES, OLDEST_INPUT_ARCHIVE_VERSION, SourceReference,
};
pub use comparison::{Comparison, WipeOrientation};
pub use compile::{CaptureLayers, CompiledView};
pub use coverage::BackingCoverage;
pub use cursor::{AffineCursor, BrushOutline, CursorIntent, ResizeDirection};
pub use damage::{DamagePlan, DirtyRegions, PixelBlit};
pub use document::{
    Document, DocumentEdit, DocumentSnapshot, EditPreview, EditSession, EditSessionId, Layer,
    LayerContent, LayerId, MaskAppearance, Provision,
};
pub use error::CanvasError;
pub use navigation::{
    Navigation, NavigationCommand, NavigationCommandOutcome, NavigationInput, ScrollPhase, ZoomMode,
};
pub use overlay::{
    ANT_DARK, ANT_DASH, ANT_LIGHT, BoundaryRun, BoundaryWork, CoverageBoundary, GUIDE,
    GUIDE_DISABLED, GUIDE_HANDLE, GUIDE_HANDLE_RADIUS, MarchingAnts, OUTLINE_DASH, OUTLINE_GAP,
    OverlayGuides, OverlayOutline, OverlaySeam, OverlayShape, PresentationOverlay, SeamHandle,
    SelectionOutlines, ToolOverlay, outline_light,
};
pub use presentation::{FrameStamp, RenderRequest};
pub use projection::ViewProjection;
pub use tools::{
    BrushMode, BrushOptions, BrushTool, CanvasTool, CoverageModification, CoverageOperation,
    CoverageShapeTool, DeferredCompletion, DeferredEdit, Disposition, ModifyPhase, ModifyTarget,
    MoveTool, PointerKind, PointerSample, PreparedEdit, Pressure, SESSION_CHECKPOINTS,
    SessionAction, SessionKind, SessionState, ShapeKind, ShapeTarget, SharedEdgesTool, ToolButton,
    ToolCancelReason, ToolCommand, ToolContext, ToolId, ToolInput, ToolManager, ToolModifiers,
    ToolOptions, ToolView, TransformAction, TransformTool,
};
pub use viewport::{MAX_SURFACE_EDGE, Viewport};
#[cfg(test)]
mod tests;
