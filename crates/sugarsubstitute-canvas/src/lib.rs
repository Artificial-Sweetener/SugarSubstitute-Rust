// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native canvas document and viewport state independent of windows and render devices.
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
pub use compile::CompiledView;
pub use coverage::BackingCoverage;
pub use damage::{DamagePlan, DirtyRegions, PixelBlit};
pub use document::{
    Document, DocumentEdit, DocumentSnapshot, EditPreview, EditSession, EditSessionId, Layer,
    LayerContent, LayerId, LayerMask,
};
pub use error::CanvasError;
pub use navigation::{
    CursorIntent, Navigation, NavigationCommand, NavigationCommandOutcome, NavigationInput,
    ScrollPhase, ZoomMode,
};
pub use presentation::{FrameStamp, RenderRequest};
pub use projection::ViewProjection;
pub use tools::{
    CanvasTool, ToolButton, ToolCancelReason, ToolContext, ToolId, ToolInput, ToolManager, ToolView,
};
pub use viewport::Viewport;
#[cfg(test)]
mod tests;
