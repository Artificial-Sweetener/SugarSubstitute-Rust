// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Device-independent frame intent and exact application-owned presentation identities.
use crate::{DamagePlan, DocumentSnapshot, Viewport};
use sugarsubstitute_contracts::execution::{Generation, SurfaceId};
use sugarsubstitute_image_ops::{Filter, Pixel};

/// One monotonically published frame on a specific native or headless canvas surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameStamp {
    /// Application-owned presentation surface identity.
    pub surface: SurfaceId,
    /// Monotonic state generation also used by execution replacement admission.
    pub generation: Generation,
}
/// One immutable document/view preparation intent with an exact presented-frame dependency.
pub struct RenderRequest<'a> {
    /// Frozen authored content for this request.
    pub document: &'a DocumentSnapshot,
    /// Requested physical camera state.
    pub view: Viewport,
    /// Raster reconstruction for this frame; preview quality never changes authored content.
    pub filter: Filter,
    /// Opaque linear-light background.
    pub matte: Pixel,
    /// Monotonic destination surface generation.
    pub target: FrameStamp,
    /// Actually presented repair base, or none for a full replacement.
    pub base: Option<FrameStamp>,
    /// Exact retained-pixel and image-preparation plan.
    pub damage: DamagePlan,
}
