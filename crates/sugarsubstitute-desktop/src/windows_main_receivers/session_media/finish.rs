// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Complete exact native media responses and check byte-range receipts before serving bytes.
use super::*;
use crate::SessionMediaRange;
impl Pending {
    pub(super) fn finish(&mut self) -> Result<(), DesktopError> {
        if !self.responded {
            let body = if self.cancelled {
                Err(SessionMediaFailure::Cancelled)
            } else {
                self.response.take().ok_or_else(|| {
                    DesktopError::message("session_media_response", "worker result absent")
                })?
            };
            let body = match body {
                Ok(MediaResponse::Metadata { .. }) if !self.head => {
                    Err(SessionMediaFailure::Failed)
                }
                Ok(MediaResponse::Data(_)) if self.head => Err(SessionMediaFailure::Failed),
                Ok(MediaResponse::Data(body))
                    if !matches_range(self.range, body.range, body.bytes.len() as u64) =>
                {
                    Err(SessionMediaFailure::Failed)
                }
                other => other,
            };
            let response = match super::super::bulk::response::media(&self.environment, body) {
                Ok(value) => value,
                Err(cause) => {
                    self.cancelled = true;
                    return Err(cause);
                }
            };
            // SAFETY: These actual retained callback args and response remain on their creating UI thread.
            if let Err(cause) = unsafe { self.args.SetResponse(&response) } {
                self.cancelled = true;
                return Err(DesktopError::new("set_session_media_response", cause));
            }
            self.responded = true;
        }
        // SAFETY: This exact retained deferral completes only after its response was actually assigned.
        unsafe { self.deferral.Complete() }
            .map_err(|cause| DesktopError::new("complete_session_media_response", cause))
    }
}
fn matches_range(
    request: Option<SessionMediaRange>,
    receipt: Option<(u64, u64)>,
    bytes: u64,
) -> bool {
    match (request, receipt) {
        (None, None) => true,
        (Some(range), Some((start, total))) => {
            let Some(end) = start.checked_add(bytes).and_then(|end| end.checked_sub(1)) else {
                return false;
            };
            if bytes == 0 || end >= total {
                return false;
            }
            match range {
                SessionMediaRange::From { start: expected, end: limit } => {
                    start == expected && limit.is_none_or(|limit| end <= limit)
                }
                SessionMediaRange::Suffix { bytes: requested } => {
                    start >= total.saturating_sub(requested) && end == total - 1
                }
            }
        }
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_range_receipt_cannot_serve_outside_the_native_request() {
        assert!(matches_range(
            Some(SessionMediaRange::From { start: 10, end: Some(19) }),
            Some((10, 100)),
            10
        ));
        assert!(!matches_range(
            Some(SessionMediaRange::From { start: 10, end: Some(19) }),
            Some((9, 100)),
            10
        ));
        assert!(!matches_range(
            Some(SessionMediaRange::From { start: 10, end: Some(19) }),
            Some((10, 100)),
            11
        ));
        assert!(matches_range(Some(SessionMediaRange::Suffix { bytes: 20 }), Some((90, 100)), 10));
        assert!(!matches_range(Some(SessionMediaRange::Suffix { bytes: 20 }), Some((70, 100)), 30));
        assert!(!matches_range(None, Some((0, 100)), 10));
        assert!(!matches_range(Some(SessionMediaRange::Suffix { bytes: 1 }), None, 1));
    }
}
