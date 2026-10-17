// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project closed console stages and direct cause codes while their original box stays owned.
use super::{Error, Kind, classify};
use crate::console_failure_diagnostic::{
    ConsoleFailureCode as Code, ConsoleFailureIoKind as IoKind,
    ConsoleFailureOperation as Operation,
};

impl Code {
    /// Inspect direct types only; neither formatting nor source callbacks are invoked.
    pub(super) fn direct(cause: &(dyn Error + 'static)) -> Self {
        #[cfg(target_os = "windows")]
        if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
            return Self::Hresult(native.code().0);
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return io.raw_os_error().map_or_else(
                || {
                    Self::Io(match classify("", io).kind {
                        Kind::TimedOut => IoKind::TimedOut,
                        Kind::Disconnected => IoKind::Disconnected,
                        Kind::Denied => IoKind::Denied,
                        Kind::InvalidInput => IoKind::InvalidInput,
                        Kind::Io | Kind::Other => IoKind::Io,
                    })
                },
                Self::Os,
            );
        }
        if cause.is::<std::num::TryFromIntError>() {
            return Self::IntegerRange;
        }
        if cause.is::<std::collections::TryReserveError>() {
            return Self::Allocation;
        }
        if cause.is::<sugarsubstitute_contracts::execution::MemoryClaimError>() {
            return Self::MemoryClaim;
        }
        Self::Opaque
    }
}

/// Enrich the existing first-failure frame only for a closed console operation.
/// Legacy fields retain their meaning; the original direct cause remains borrowed and owned.
pub(super) fn emit(operation: &'static str, cause: &(dyn Error + 'static)) -> bool {
    let Some(operation) = Operation::from_operation(operation) else { return false };
    let actual = classify("", cause);
    let code = Code::direct(cause);
    tracing::warn!(operation = "desktop_failure_transition", boundary = ?actual.boundary,
        kind = ?actual.kind, console_operation = ?operation, console_code = ?code,
        "First owned desktop failure retained");
    true
}
