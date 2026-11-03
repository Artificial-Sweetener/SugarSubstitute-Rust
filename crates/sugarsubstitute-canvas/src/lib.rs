// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native canvas document and viewport state independent of windows and render devices.
mod comparison;
mod compile;
mod coverage;
mod damage;
mod document;
mod error;
mod navigation;
mod presentation;
mod projection;
mod tools;
mod viewport;
pub use comparison::{Comparison, WipeOrientation};
pub use compile::CompiledView;
pub use coverage::BackingCoverage;
pub use damage::{DamagePlan, DirtyRegions, PixelBlit};
pub use document::{
    Document, DocumentEdit, DocumentSnapshot, EditPreview, EditSession, EditSessionId, Layer,
    LayerContent, LayerId, MaskAppearance,
};
pub use error::CanvasError;
pub use navigation::{
    CursorIntent, Navigation, NavigationCommand, NavigationCommandOutcome, NavigationInput,
    ScrollPhase, ZoomMode,
};
pub use presentation::{FrameStamp, RenderRequest};
pub use projection::ViewProjection;
pub use tools::{
    CanvasTool, PointerKind, PointerSample, Pressure, ToolButton, ToolCancelReason, ToolContext,
    ToolId, ToolInput, ToolManager, ToolModifiers, ToolView,
};
pub use viewport::{MAX_SURFACE_EDGE, Viewport};
#[cfg(test)]
mod tests;
