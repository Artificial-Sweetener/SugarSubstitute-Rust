// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit normal outer-window placement independently of any OS window or monitor handle.
use super::{SessionError, wire::SessionPlacement};

/// Validate bounded DPI-normalized normal OUTER bounds and a native monitor identifier.
///
/// `bounds` and `work_area` positions/extents are physical SCREEN values divided uniformly by
/// captured reference-monitor scale `dpi / 96`. Native restore computes logical offset
/// `bounds.xy - work_area.xy`, then target physical position as target work-area origin
/// plus offset times target scale; outer size is saved logical size times target scale.
/// The desktop owner matches/falls back to a current monitor, clamps against its work area
/// and minimum size, and applies normal bounds before reveal/maximization. Capture retains
/// the last normal outer bounds across maximization/minimization; minimized is never restored.
/// This pure admission grants no native access and has no thread affinity.
/// # Errors
/// Rejects invalid shape/bounds/DPI or empty/control-bearing monitor names, including NUL.
pub fn validate_placement(value: &SessionPlacement) -> Result<(), SessionError> {
    if !value.valid() || value.monitor.is_empty() || value.monitor.chars().any(char::is_control) {
        return Err(SessionError::Invalid);
    }
    Ok(())
}
