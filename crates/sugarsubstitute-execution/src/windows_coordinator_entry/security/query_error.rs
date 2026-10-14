// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve kernel-query peer loss as a terminal disposition for only its connected slot.
use crate::{coordinator_entry::EntryTransportError, windows::NativeObservationError};
use windows::{Win32::Foundation::ERROR_PIPE_NOT_CONNECTED, core::HRESULT};
pub(super) fn classify(cause: windows::core::Error) -> EntryTransportError {
    let disconnected = cause.code() == HRESULT::from_win32(ERROR_PIPE_NOT_CONNECTED.0);
    let original = NativeObservationError::new("entry_query_kernel_peer", cause);
    if disconnected {
        EntryTransportError::PeerDisconnected(original)
    } else {
        EntryTransportError::native(original)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use windows::Win32::Foundation::*;
    #[test]
    fn only_disconnected_kernel_query_is_local_and_retains_exact_hresult() {
        for code in [
            ERROR_PIPE_NOT_CONNECTED,
            ERROR_ACCESS_DENIED,
            ERROR_BROKEN_PIPE,
            ERROR_INVALID_HANDLE,
            ERROR_NO_DATA,
        ] {
            let original = HRESULT::from_win32(code.0);
            let result = classify(windows::core::Error::from_hresult(original));
            assert_eq!(
                matches!(result, EntryTransportError::PeerDisconnected(_)),
                code == ERROR_PIPE_NOT_CONNECTED
            );
            assert_eq!(
                result.native_cause().map(NativeObservationError::stage),
                Some("entry_query_kernel_peer")
            );
            assert_eq!(
                result
                    .native_cause()
                    .and_then(Error::source)
                    .and_then(|e| e.downcast_ref::<windows::core::Error>())
                    .map(windows::core::Error::code),
                Some(original)
            );
        }
    }
}
