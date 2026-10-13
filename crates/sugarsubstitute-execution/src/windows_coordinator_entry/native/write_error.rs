// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only an unaccepted write to a requester that has closed its pipe.
use crate::{coordinator_entry::EntryTransportError, native_pipe::NativePipeError};
use std::error::Error;
use windows::Win32::Foundation::ERROR_NO_DATA;

pub(super) fn classify(error: NativePipeError) -> EntryTransportError {
    if !error.identity_unrestored
        && !error.cause.contains_owner_panic()
        && error.cause.stage() == "pipe_issue"
        && error
            .cause
            .source()
            .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
            .is_some_and(|cause| crate::native_pipe::handles::code(cause, ERROR_NO_DATA))
    {
        EntryTransportError::PeerDisconnected(error.cause)
    } else {
        error.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_pipe::handles::error;
    use windows::{Win32::Foundation::*, core::HRESULT};

    #[test]
    fn only_unaccepted_no_data_write_is_connection_local_and_preserves_native_cause() {
        let native = windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_NO_DATA.0));
        let local = classify(error("pipe_issue", native));
        assert!(matches!(local, EntryTransportError::PeerDisconnected(_)));
        assert_eq!(
            local
                .native_cause()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
                .map(|cause| cause.code()),
            Some(HRESULT::from_win32(ERROR_NO_DATA.0))
        );
        for code in [ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE] {
            let native = windows::core::Error::from_hresult(HRESULT::from_win32(code.0));
            assert!(matches!(
                classify(error("pipe_issue", native)),
                EntryTransportError::Native { .. }
            ));
        }
        let native = windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_NO_DATA.0));
        assert!(matches!(
            classify(error("pipe_completion", native)),
            EntryTransportError::Native { .. }
        ));
        let native = windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_NO_DATA.0));
        let mut fatal = error("pipe_issue", native);
        fatal.identity_unrestored = true;
        assert!(classify(fatal).requires_process_shutdown());
    }
}
