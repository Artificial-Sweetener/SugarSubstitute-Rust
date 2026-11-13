// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Select the system-wide monotonic counter that native peers share with this process.
#[cfg(not(any(windows, unix)))]
use crate::native::NativeObservationError;

#[cfg(unix)]
pub(crate) use crate::unix_counter::sample;
#[cfg(windows)]
pub(crate) use crate::windows_counter::sample;

/// No shared system counter is declared for this platform; deadlines cannot cross processes.
#[cfg(not(any(windows, unix)))]
pub(crate) fn sample() -> Result<(u64, u64), NativeObservationError> {
    Err(NativeObservationError::new(
        "execution_system_counter",
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "No system-wide monotonic counter is supported on this platform",
        ),
    ))
}
