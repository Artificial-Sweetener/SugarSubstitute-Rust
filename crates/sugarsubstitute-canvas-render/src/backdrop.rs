// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded presentation-only acrylic sources sampled from a completed display projection.
//! Regions retain physical viewport coordinates; no operation edits a document or canonical
//! backing. The application retains frame, geometry and source-generation publication authority.
mod cpu;
mod gpu;
mod plan;
pub use gpu::{GpuBackdrop, GpuBackdropPatch, GpuBackdropPatches};
pub use plan::{BackdropCrop, BackdropGraph, BackdropPlan, BackdropRegion};

#[cfg(test)]
mod tests;
