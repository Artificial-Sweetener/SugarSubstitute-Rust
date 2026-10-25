// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native canvas document and viewport state independent of windows and render devices.
mod compile;
mod damage;
mod document;
mod error;
mod presentation;
mod viewport;
pub use compile::CompiledView;
pub use damage::{DamagePlan, DirtyRegions, PixelBlit};
pub use document::{
    Document, DocumentEdit, DocumentSnapshot, Layer, LayerContent, LayerId, LayerMask,
};
pub use error::CanvasError;
pub use presentation::{FrameStamp, RenderRequest};
pub use viewport::Viewport;
#[cfg(test)]
mod tests;
