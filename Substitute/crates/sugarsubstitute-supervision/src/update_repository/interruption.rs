// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide whether an early-ended package response resumes within the current attempt.
use crate::{setup_storage::DownloadFailure, update_http};
use std::{error::Error, io};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::delivery::Rejection;

/// `HRESULT_FROM_WIN32(ERROR_WINHTTP_TIMEOUT)`: WinHTTP's idle bound elapsed with no bytes.
const WINHTTP_TIMEOUT: i32 = 0x8007_2EE2_u32 as i32;

/// A response resumes only when it durably extended the prefix and ended for a reason that a
/// fresh ranged request can overcome: a dropped connection, or the job deadline reached while
/// bytes were still arriving. A stall (the idle timeout), cancellation, framing or retirement
/// failures, and interruptions that delivered nothing end the attempt.
pub(super) fn resumes(cause: &update_http::Failure, advanced: bool, context: &JobContext) -> bool {
    if !advanced || context.cancellation_requested() || !transfer(cause) {
        return false;
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return true;
    }
    !stalled(cause)
}

/// Only transport I/O, partial-file I/O and a body shorter than its framing (the peer closed the
/// connection early) describe an interrupted body. Everything else rejects the response or
/// retained custody and must not be retried silently. Every resumed byte is still covered by
/// the final digest verification, and each resume must extend the prefix, so resumption of a
/// hostile response is bounded by the authenticated package length.
fn transfer(cause: &update_http::Failure) -> bool {
    matches!(
        cause,
        update_http::Failure::Io(_)
            | update_http::Failure::Download(
                DownloadFailure::Storage(_) | DownloadFailure::Response(Rejection::BodyLength)
            )
    )
}

/// Walk the retained cause chain for the native idle timeout or an I/O timeout classification.
fn stalled(cause: &update_http::Failure) -> bool {
    let mut current: Option<&(dyn Error + 'static)> = Some(cause);
    while let Some(error) = current {
        if error
            .downcast_ref::<windows::core::Error>()
            .is_some_and(|native| native.code().0 == WINHTTP_TIMEOUT)
        {
            return true;
        }
        if let Some(io) = error.downcast_ref::<io::Error>() {
            if io.kind() == io::ErrorKind::TimedOut {
                return true;
            }
            if let Some(inner) = io.get_ref() {
                current = Some(inner);
                continue;
            }
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native(code: i32) -> update_http::Failure {
        update_http::Failure::Download(DownloadFailure::Storage(io::Error::other(
            windows::core::Error::from_hresult(windows::core::HRESULT(code)),
        )))
    }

    #[test]
    fn native_idle_timeout_and_timed_out_io_are_stalls_while_resets_are_not() {
        assert!(stalled(&native(WINHTTP_TIMEOUT)));
        assert!(stalled(&update_http::Failure::Io(io::ErrorKind::TimedOut.into())));
        // ERROR_WINHTTP_CONNECTION_ERROR: the peer closed or reset the connection.
        assert!(!stalled(&native(0x8007_2EFE_u32 as i32)));
        assert!(!stalled(&update_http::Failure::Download(DownloadFailure::Storage(
            io::ErrorKind::ConnectionReset.into()
        ))));
    }

    #[test]
    fn framing_policy_and_retirement_failures_never_describe_a_resumable_transfer() {
        assert!(!transfer(&update_http::Failure::Policy(
            sugarsubstitute_update::http::Rejection::Status(500)
        )));
        assert!(!transfer(&update_http::Failure::Retirement {
            operation: Box::new(update_http::Failure::Io(io::ErrorKind::ConnectionReset.into())),
            retirement: io::ErrorKind::Other.into(),
        }));
        assert!(!transfer(&update_http::Failure::Download(DownloadFailure::Response(
            Rejection::Offset
        ))));
        assert!(transfer(&update_http::Failure::Io(io::ErrorKind::ConnectionReset.into())));
        assert!(transfer(&update_http::Failure::Download(DownloadFailure::Response(
            Rejection::BodyLength
        ))));
    }
}
