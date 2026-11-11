// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native placement observations bounded and resolve saved logical geometry onto current monitors.

use crate::MainIdentity;
use std::num::NonZeroU64;
use sugarsubstitute_contracts::desktop_session::wire::SessionPlacement;

#[cfg(any(target_os = "windows", test))]
mod policy;
#[cfg(target_os = "windows")]
pub(crate) use policy::*;

/// One actual creating-thread placement sample; neither identity nor geometry grants native access.
#[derive(Debug)]
pub struct MainPlacementObservation {
    /// Exact retained native lifetime, including navigation generation.
    pub identity: MainIdentity,
    /// Strictly increasing sequence within that native lifetime.
    pub sequence: NonZeroU64,
    /// Explicit capture correlation; ordinary coalesced changes have no request identity.
    pub capture_id: Option<NonZeroU64>,
    /// Normal outer screen bounds normalized by the recorded monitor's DPI.
    pub placement: SessionPlacement,
}
