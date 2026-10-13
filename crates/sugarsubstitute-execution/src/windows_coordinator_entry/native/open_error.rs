// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only actual absence at the unaccepted local client open boundary.
use crate::{coordinator_entry::EntryTransportError, native::NativeObservationError};
use windows::{Win32::Foundation::ERROR_FILE_NOT_FOUND, core::Error};

pub(super) fn classify(cause: Error) -> EntryTransportError {
    let absent = crate::native_pipe::handles::code(&cause, ERROR_FILE_NOT_FOUND);
    let cause = NativeObservationError::new("entry_open_client", cause);
    if absent {
        EntryTransportError::EndpointAbsent(cause)
    } else {
        EntryTransportError::native(cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    use windows::{Win32::Foundation::*, core::HRESULT};

    #[test]
    fn only_missing_client_open_is_expected_and_keeps_exact_native_cause() {
        let missing = classify(Error::from_hresult(HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)));
        assert!(matches!(missing, EntryTransportError::EndpointAbsent(_)));
        assert_eq!(
            missing.native_cause().map(NativeObservationError::stage),
            Some("entry_open_client")
        );
        assert_eq!(
            missing
                .native_cause()
                .and_then(|cause| cause.source())
                .and_then(|cause| cause.downcast_ref::<Error>())
                .map(Error::code),
            Some(HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0))
        );
        for code in [ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, ERROR_BROKEN_PIPE, ERROR_INVALID_HANDLE]
        {
            assert!(matches!(
                classify(Error::from_hresult(HRESULT::from_win32(code.0))),
                EntryTransportError::Native { .. }
            ));
        }
    }
}
