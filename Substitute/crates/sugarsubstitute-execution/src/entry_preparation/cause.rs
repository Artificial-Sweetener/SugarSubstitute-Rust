// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep categorical raw capture failures and genuine causes separate from last-error observations.

use super::control::EntryPreparationControlCause;
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::{coordinator_entry::EntryShapeError, execution::PanicCause};

/// Preserve an actual environment-copy or checked-retirement failure.
/// Formatting redacts native messages, environment values and panic payloads.
pub enum EntryEnvironmentCause {
    /// The getter returned NULL; its immediately observed last-error value is
    /// retained as data, without a documented GetLastError guarantee for Get.
    GetNull {
        /// Immediate raw observation, including zero; it may be unrelated.
        last_error_observation: u32,
    },
    /// No original double NUL occurred within the copied-unit policy.
    TooLarge {
        /// Number of actual units inspected, including any terminal zeros.
        inspected_units: usize,
    },
    /// Preserve the actual fallible copy allocation cause.
    CopyAllocation {
        /// Original allocator rejection.
        source: TryReserveError,
    },
    /// The allocator's actual retained capacity exceeded policy.
    CopyCapacity {
        /// Retained UTF16 capacity in units, independently of length.
        actual_capacity_units: usize,
    },
    /// Preserve the authoritative Domain launch validator's rejection.
    Shape {
        /// Original request grammar or retained-capacity failure.
        source: EntryShapeError,
    },
    /// Preserve a genuine original control observation.
    Control {
        /// Actual original cancellation/clock/cutoff or provider panic.
        source: EntryPreparationControlCause,
    },
    /// Keep an actual association rejection without allocating an extra cause box.
    Preparation {
        /// Original nonclone source, protected by the retaining preparation owner.
        source: super::EntryPreparationInputError,
    },
    /// Preserve the documented checked Unicode Free error.
    #[cfg(windows)]
    Free {
        /// Actual failed Win32 BOOL/GetLastError cause.
        source: windows::core::Error,
    },
    /// The owned native allocation is unavailable for a requested operation.
    /// This category grants no release, retry or settlement authority.
    InvalidNativeState,
    /// Preserve a bounded actual external-provider panic.
    ProviderPanic {
        /// Original panic data; never automatically logged.
        source: PanicCause,
    },
}
impl EntryEnvironmentCause {
    /// Identify the actual category without formatting native or panic data.
    pub const fn category(&self) -> &'static str {
        match self {
            Self::GetNull { .. } => "get_null",
            Self::TooLarge { .. } => "too_large",
            Self::CopyAllocation { .. } => "copy_allocation",
            Self::CopyCapacity { .. } => "copy_capacity",
            Self::Shape { .. } => "shape",
            Self::Control { .. } => "control",
            Self::Preparation { .. } => "preparation",
            #[cfg(windows)]
            Self::Free { .. } => "free",
            Self::InvalidNativeState => "invalid_native_state",
            Self::ProviderPanic { .. } => "provider_panic",
        }
    }
    /// Borrow actual panic data for explicit fatal handling, not authority inference.
    pub const fn panic_cause(&self) -> Option<&PanicCause> {
        match self {
            Self::ProviderPanic { source } => Some(source),
            Self::Control { source } => source.panic_cause(),
            _ => None,
        }
    }
}
impl fmt::Debug for EntryEnvironmentCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryEnvironmentCause")
            .field("category", &self.category())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for EntryEnvironmentCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entry environment preparation failed ({})", self.category())
    }
}
impl Error for EntryEnvironmentCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CopyAllocation { source } => Some(source),
            Self::Shape { source } => Some(source),
            Self::Control { source } => Some(source),
            Self::Preparation { source } => Some(source),
            #[cfg(windows)]
            Self::Free { source } => Some(source),
            _ => None,
        }
    }
}
