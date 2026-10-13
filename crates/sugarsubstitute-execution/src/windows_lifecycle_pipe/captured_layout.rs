// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preflight captured metadata partitions from actual page facts and compiled owner sizes.

use crate::lifecycle_pipe::{CAPTURED_METADATA_BYTES, LifecyclePipeFailure, failure};

pub(super) const STARTUP_LOGICAL_BYTES: usize = 67_480;
const MAP_CONTROL_BYTES: usize = 4_096;

/// Exact original captured metadata split; neither claim increases the admitted total.
pub(super) struct CapturedLayout {
    pub(super) map_extent: usize,
    pub(super) map_bytes: usize,
    pub(super) native_bytes: usize,
}

impl CapturedLayout {
    /// Validate actual GetSystemInfo page size and every compiled owner bound before mapping.
    /// `native_storage` is the actual joined Active/Shared/lease/security/output layout;
    /// `map_storage` covers both controls, loan values and their retained Signal control.
    pub(super) fn new(
        page: usize,
        map_storage: usize,
        native_storage: usize,
    ) -> Result<Self, LifecyclePipeFailure> {
        if !page.is_power_of_two() || map_storage > MAP_CONTROL_BYTES {
            return Err(failure("captured_mapping_layout"));
        }
        let map_extent = STARTUP_LOGICAL_BYTES
            .checked_add(page - 1)
            .map(|n| n & !(page - 1))
            .ok_or_else(|| failure("captured_mapping_layout"))?;
        let map_bytes = map_extent
            .checked_add(MAP_CONTROL_BYTES)
            .ok_or_else(|| failure("captured_mapping_layout"))?;
        let native_bytes = CAPTURED_METADATA_BYTES
            .checked_sub(map_bytes)
            .filter(|n| *n >= native_storage)
            .ok_or_else(|| failure("captured_native_layout"))?;
        Ok(Self { map_extent, map_bytes, native_bytes })
    }
}
