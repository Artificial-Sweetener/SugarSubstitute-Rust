// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own finite positive Windows QPF/QPC queries shared by every execution consumer.
use crate::native::NativeObservationError;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

pub(crate) fn sample() -> Result<(u64, u64), NativeObservationError> {
    let mut frequency = 0;
    let mut ticks = 0;
    // SAFETY: finite queries write only these live signed integer outputs.
    unsafe { QueryPerformanceFrequency(&mut frequency) }
        .map_err(|cause| NativeObservationError::new("execution_qpc_frequency", cause))?;
    // SAFETY: the signed output remains live for this finite native query.
    unsafe { QueryPerformanceCounter(&mut ticks) }
        .map_err(|cause| NativeObservationError::new("execution_qpc_counter", cause))?;
    if frequency <= 0 || ticks <= 0 {
        return Err(NativeObservationError::new(
            "execution_positive_counter",
            std::io::Error::other("QPC/QPF returned nonpositive output"),
        ));
    }
    Ok((ticks as u64, frequency as u64))
}
