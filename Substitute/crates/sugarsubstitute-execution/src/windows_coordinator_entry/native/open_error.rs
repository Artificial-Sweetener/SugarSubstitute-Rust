// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only actual absence or occupancy at the unaccepted local client open boundary.
use crate::{coordinator_entry::EntryTransportError, native::NativeObservationError};
use windows::{
    Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY},
    core::Error,
};

pub(super) fn classify(cause: Error) -> EntryTransportError {
    let absent = crate::native_pipe::handles::code(&cause, ERROR_FILE_NOT_FOUND);
    // Every listening instance is held by another peer, such as a concurrent launch probing
    // the same coordinator. The endpoint exists, so this is neither absence nor a fault.
    let busy = crate::native_pipe::handles::code(&cause, ERROR_PIPE_BUSY);
    let cause = NativeObservationError::new("entry_open_client", cause);
    if absent {
        EntryTransportError::EndpointAbsent(cause)
    } else if busy {
        EntryTransportError::EndpointBusy(cause)
    } else {
        EntryTransportError::native(cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    use windows::{Win32::Foundation::*, core::HRESULT};

    fn exact_code(error: &EntryTransportError) -> Option<HRESULT> {
        error
            .native_cause()
            .and_then(|cause| cause.source())
            .and_then(|cause| cause.downcast_ref::<Error>())
            .map(Error::code)
    }

    #[test]
    fn only_missing_or_occupied_client_open_is_expected_and_keeps_exact_native_cause() {
        let missing = classify(Error::from_hresult(HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)));
        assert!(matches!(missing, EntryTransportError::EndpointAbsent(_)));
        assert_eq!(
            missing.native_cause().map(NativeObservationError::stage),
            Some("entry_open_client")
        );
        assert_eq!(exact_code(&missing), Some(HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)));
        let busy = classify(Error::from_hresult(HRESULT::from_win32(ERROR_PIPE_BUSY.0)));
        assert!(matches!(busy, EntryTransportError::EndpointBusy(_)));
        assert_eq!(
            busy.native_cause().map(NativeObservationError::stage),
            Some("entry_open_client")
        );
        assert_eq!(exact_code(&busy), Some(HRESULT::from_win32(ERROR_PIPE_BUSY.0)));
        assert!(!busy.requires_process_shutdown());
        for code in [ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_INVALID_HANDLE] {
            assert!(matches!(
                classify(Error::from_hresult(HRESULT::from_win32(code.0))),
                EntryTransportError::Native { .. }
            ));
        }
    }
}
