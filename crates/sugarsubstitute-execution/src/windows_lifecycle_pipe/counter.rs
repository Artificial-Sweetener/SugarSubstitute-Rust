// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Query the actual positive Windows counter without waiting or sampling application state.
use crate::native::NativeObservationError;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
pub(crate) fn sample() -> Result<(u64, u64), NativeObservationError> {
    let mut frequency = 0;
    let mut ticks = 0;
    // SAFETY: finite native queries write only these live signed integer outputs.
    unsafe { QueryPerformanceFrequency(&mut frequency) }
        .map_err(|e| NativeObservationError::new("lifecycle_qpc_frequency", e))?;
    // SAFETY: the signed output remains live for this finite query.
    unsafe { QueryPerformanceCounter(&mut ticks) }
        .map_err(|e| NativeObservationError::new("lifecycle_qpc_counter", e))?;
    if frequency <= 0 || ticks <= 0 {
        return Err(NativeObservationError::new(
            "lifecycle_positive_clock",
            std::io::Error::other("QPC/QPF returned nonpositive output"),
        ));
    }
    Ok((ticks as u64, frequency as u64))
}
