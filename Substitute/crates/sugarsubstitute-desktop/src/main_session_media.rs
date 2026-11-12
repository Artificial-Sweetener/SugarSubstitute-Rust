// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer retained-media requests and bounded immutable replies without exposing file paths.
use crate::{DesktopError, MainIdentity};
use std::{num::NonZeroU64, time::Instant};
use sugarsubstitute_contracts::{desktop_session::wire::SessionBinding, execution::MemoryClaim};
#[cfg(any(windows, test))]
pub(crate) mod route;

/// Maximum encoded original handed to the browser response owner, in bytes.
pub const MAX_MEDIA_BYTES: usize = 64 * 1024 * 1024;
/// One syntactically valid byte range; the trusted asset owner applies its actual total length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMediaRange {
    /// Inclusive start and optional inclusive end (`None` means the remaining original).
    From {
        /// First original byte, counted from zero.
        start: u64,
        /// Inclusive final byte, or the original's end when absent.
        end: Option<u64>,
    },
    /// Last nonzero `bytes` of the original.
    Suffix {
        /// Nonzero count of original bytes requested from its end.
        bytes: u64,
    },
}
impl SessionMediaRange {
    /// Parse exactly one bounded HTTP byte range; no whitespace, multiple ranges or overflow.
    /// # Errors
    /// Refuses malformed/unsatisfiable syntax before any native or file effect.
    #[cfg(any(windows, test))]
    pub(crate) fn parse(value: &str) -> Result<Self, DesktopError> {
        let refuse = || DesktopError::message("session_media_range", "invalid single byte range");
        if value.len() > 64 {
            return Err(refuse());
        }
        let value = value.strip_prefix("bytes=").ok_or_else(refuse)?;
        let (start, end) = value.split_once('-').ok_or_else(refuse)?;
        let number = |text: &str| -> Result<u64, DesktopError> {
            if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(refuse());
            }
            text.parse().map_err(|_| refuse())
        };
        if start.is_empty() {
            let bytes = number(end)?;
            if bytes == 0 {
                return Err(refuse());
            }
            return Ok(Self::Suffix { bytes });
        }
        let start = number(start)?;
        let end = (!end.is_empty()).then(|| number(end)).transpose()?;
        if end.is_some_and(|end| end < start) {
            return Err(refuse());
        }
        Ok(Self::From { start, end })
    }
}
/// Why a retained asset cannot be served; the original cause remains with its worker owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMediaFailure {
    /// The recorded original is unavailable or no exact identity could be proven.
    Unavailable,
    /// The page, checkpoint, workflow generation or navigation was superseded.
    Stale,
    /// The encoded original exceeds the browser response admission.
    TooLarge,
    /// Its original request was cancelled or expired.
    Cancelled,
    /// A storage or native boundary failed with a retained original cause.
    Failed,
}
/// One GET or HEAD observed in the current independently admitted browser lifetime.
pub struct SessionMediaObservation {
    identity: MainIdentity,
    request_id: NonZeroU64,
    webview: String,
    output_id: String,
    binding: SessionBinding,
    deadline: Instant,
    range: Option<SessionMediaRange>,
    head: bool,
    _claim: MemoryClaim,
}
impl SessionMediaObservation {
    #[cfg(windows)]
    pub(crate) fn new(
        identity: MainIdentity,
        request_id: NonZeroU64,
        webview: String,
        requested: route::Requested,
        deadline: Instant,
        claim: MemoryClaim,
    ) -> Self {
        Self {
            identity,
            request_id,
            webview,
            output_id: requested.output,
            binding: requested.binding,
            deadline,
            range: requested.range,
            head: requested.head,
            _claim: claim,
        }
    }
    /// Return the actual native main lifetime of this request.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Return the native per-lifetime request identity.
    pub const fn request_id(&self) -> NonZeroU64 {
        self.request_id
    }
    /// Borrow the host-granted current webview UUID.
    pub fn webview(&self) -> &str {
        &self.webview
    }
    /// Borrow the requested output correlation UUID; it grants no file authority.
    pub fn output_id(&self) -> &str {
        &self.output_id
    }
    /// Borrow the exact requested occurrence/runtime/revision binding.
    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }
    /// Return the original native request cutoff; callers must never renew it.
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Return one native-observed byte range; no filesystem length is inferred here.
    pub const fn range(&self) -> Option<SessionMediaRange> {
        self.range
    }
    /// Whether this is a HEAD metadata request, which must not return a body.
    pub const fn head(&self) -> bool {
        self.head
    }
}
/// A charged immutable encoded response, or explicit unavailable disposition.
pub struct SessionMediaReply {
    identity: MainIdentity,
    request_id: NonZeroU64,
    body: Result<MediaResponse, SessionMediaFailure>,
}
pub(crate) enum MediaResponse {
    Data(MediaBody),
    Metadata { mime: &'static str, bytes: u64, animated: bool },
}
pub(crate) struct MediaBody {
    pub(crate) mime: &'static str,
    pub(crate) bytes: Vec<u8>,
    pub(crate) claim: MemoryClaim,
    pub(crate) range: Option<(u64, u64)>,
}
impl SessionMediaReply {
    /// Adopt a worker-produced exact original with its existing allocation charge.
    /// # Errors
    /// Refuses empty/excessive/underfunded buffers or unsupported MIME without native effects.
    pub fn new(
        identity: MainIdentity,
        request_id: NonZeroU64,
        mime: &str,
        bytes: Vec<u8>,
        claim: MemoryClaim,
    ) -> Result<Self, DesktopError> {
        let mime = mime_type(mime)?;
        if bytes.is_empty()
            || bytes.len() > MAX_MEDIA_BYTES
            || bytes.capacity() > MAX_MEDIA_BYTES
            || claim.bytes() < bytes.capacity() as u64
        {
            return Err(DesktopError::message(
                "session_media_reply",
                "encoded media admission refused",
            ));
        }
        Ok(Self {
            identity,
            request_id,
            body: Ok(MediaResponse::Data(MediaBody { mime, bytes, claim, range: None })),
        })
    }
    /// Adopt one bounded partial response. `start` is inclusive and `total` the verified
    /// original length; native response construction computes the exact inclusive end.
    /// # Errors
    /// Refuses the same buffer/MIME violations as `new`, or a range outside the verified total.
    pub fn partial(
        identity: MainIdentity,
        request_id: NonZeroU64,
        mime: &str,
        bytes: Vec<u8>,
        claim: MemoryClaim,
        start: u64,
        total: u64,
    ) -> Result<Self, DesktopError> {
        if total == 0
            || total > sugarsubstitute_contracts::desktop_session::MAX_ASSET_BYTES
            || start.checked_add(bytes.len() as u64).is_none_or(|end| end > total)
        {
            return Err(DesktopError::message(
                "session_media_reply",
                "partial body exceeds original",
            ));
        }
        let mut result = Self::new(identity, request_id, mime, bytes, claim)?;
        if let Ok(MediaResponse::Data(body)) = &mut result.body {
            body.range = Some((start, total));
        }
        Ok(result)
    }
    /// Return verified exact-original metadata without allocating or transferring media bytes.
    /// The worker verifies digest and current media header before constructing this receipt.
    /// # Errors
    /// Refuses unsupported MIME or an original length outside authoritative retained-asset bounds.
    pub fn metadata(
        identity: MainIdentity,
        request_id: NonZeroU64,
        mime: &str,
        bytes: u64,
        animated: bool,
    ) -> Result<Self, DesktopError> {
        if bytes == 0 || bytes > sugarsubstitute_contracts::desktop_session::MAX_ASSET_BYTES {
            return Err(DesktopError::message(
                "session_media_metadata",
                "original length exceeds asset bound",
            ));
        }
        Ok(Self {
            identity,
            request_id,
            body: Ok(MediaResponse::Metadata { mime: mime_type(mime)?, bytes, animated }),
        })
    }
    /// Complete an unavailable request while preserving its original deadline and identity.
    pub const fn unavailable(
        identity: MainIdentity,
        request_id: NonZeroU64,
        failure: SessionMediaFailure,
    ) -> Self {
        Self { identity, request_id, body: Err(failure) }
    }
    /// Return the addressed actual native main lifetime.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Return the original per-lifetime native request identity.
    pub const fn request_id(&self) -> NonZeroU64 {
        self.request_id
    }
    /// Return the encoded response MIME or the explicit unavailable disposition.
    pub fn content_type(&self) -> Result<&'static str, SessionMediaFailure> {
        match &self.body {
            Ok(MediaResponse::Data(body)) => Ok(body.mime),
            Ok(MediaResponse::Metadata { mime, .. }) => Ok(mime),
            Err(cause) => Err(*cause),
        }
    }
    /// Return response-body bytes, verified original bytes, and the retained allocation charge.
    /// HEAD has zero response bytes; a partial body preserves its original total.
    pub fn byte_counts(&self) -> Result<(u64, u64, u64), SessionMediaFailure> {
        match &self.body {
            Ok(MediaResponse::Data(body)) => {
                let bytes = body.bytes.len() as u64;
                Ok((bytes, body.range.map_or(bytes, |(_, total)| total), body.claim.bytes()))
            }
            Ok(MediaResponse::Metadata { bytes, .. }) => Ok((0, *bytes, 0)),
            Err(cause) => Err(*cause),
        }
    }
    /// Borrow worker-verified animation metadata for a HEAD reply, when present.
    pub fn animated(&self) -> Option<bool> {
        match &self.body {
            Ok(MediaResponse::Metadata { animated, .. }) => Some(*animated),
            _ => None,
        }
    }
    #[cfg(windows)]
    pub(crate) fn into_body(self) -> Result<MediaResponse, SessionMediaFailure> {
        self.body
    }
}
fn mime_type(mime: &str) -> Result<&'static str, DesktopError> {
    match mime {
        "image/png" => Ok("image/png"),
        "image/jpeg" => Ok("image/jpeg"),
        "image/webp" => Ok("image/webp"),
        "image/gif" => Ok("image/gif"),
        "video/mp4" => Ok("video/mp4"),
        "video/webm" => Ok("video/webm"),
        _ => Err(DesktopError::message("session_media_reply", "unsupported media type")),
    }
}
#[cfg(test)]
mod tests;
