// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry oversized panel messages through single-use tokens on one host-owned origin.
//!
//! The page context is untrusted: other extensions share it, so the bulk origin moves only
//! exact JSON bytes and grants nothing. An uploaded body re-enters the same framing, ticket,
//! session and decode admission as an inline lane message. A downloaded body reaches the
//! bridge's unchanged reply or notification validation. Every token is 128 random bits,
//! single-use and bound to the current panel document; bodies are claimed from a dedicated
//! partition of the main reservation so in-flight bytes stay within [`SESSION_BYTES`].
//!
//! All state is UI-affine. Callers pass the observation instant so expiry is deterministic.

use crate::{DesktopError, MainWindowRequest, main_panel_ingress::Ticket};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryClaimError, MemoryLease};

mod wire;
pub(crate) use wire::{Envelope, Route, Token};

/// The reserved origin; `.invalid` never resolves, so only the host's handler can answer it.
pub(crate) const ORIGIN: &str = "https://sugarsubstitute-bulk.invalid/";
/// The WebView2 request filter that routes every bulk-origin fetch to the host's handler.
#[cfg(target_os = "windows")]
pub(crate) const FILTER: &str = "https://sugarsubstitute-bulk.invalid/*";
/// Largest serialized message carried in either direction, in UTF-8 bytes.
///
/// A 64 MiB raster is 89,478,488 base64 characters; the remainder covers its JSON.
pub(crate) const MAX_MESSAGE_BYTES: usize = 96 * 1024 * 1024;
/// Bytes one panel session may hold in flight: one maximum message in each direction.
pub(crate) const SESSION_BYTES: u64 = MainWindowRequest::BULK_TRANSFER_BYTES;
/// Undelivered host messages retained at once; later oversized messages are refused.
const MAX_DOWNLOADS: usize = 4;
/// Time an uploaded or staged body may wait for its single matching use.
const LIFETIME: Duration = Duration::from_secs(30);
/// Upload storage is claimed in fixed steps so a short body never reserves the maximum.
const UPLOAD_STEP: usize = 4 * 1024 * 1024;
/// Largest single native read into upload storage.
const READ_BYTES: usize = 1024 * 1024;
/// Why an oversized message could not take the bulk path; the caller fails closed.
#[derive(Debug)]
pub(crate) enum Refusal {
    /// The panel document changed or the window is closing.
    Closed,
    /// The body is empty, exceeds [`MAX_MESSAGE_BYTES`] or is not UTF-8.
    Size,
    /// Too many staged messages await retrieval.
    Capacity,
    /// The session's in-flight byte partition is exhausted.
    Memory(MemoryClaimError),
    /// The native request body could not be read.
    Read,
    /// Token generation or envelope encoding failed natively.
    Native(DesktopError),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Closed => "Bulk transfer belongs to a closed panel document",
            Self::Size => "Bulk transfer size or encoding is outside its bound",
            Self::Capacity => "Bulk transfer retention is exhausted",
            Self::Memory(_) => "Bulk transfer partition is exhausted",
            Self::Read => "Bulk transfer body could not be read",
            Self::Native(_) => "Bulk transfer token or envelope could not be produced",
        })
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Memory(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Closed | Self::Size | Self::Capacity | Self::Read => None,
        }
    }
}

/// A received upload awaiting its lane envelope; its storage claims drop with it.
pub(crate) struct Upload {
    text: String,
    _claims: Vec<MemoryClaim>,
}

impl Upload {
    /// Borrow the exact uploaded JSON while its claims remain held.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

/// A staged host message transferred to exactly one native response.
pub(crate) struct Download {
    bytes: Vec<u8>,
    claim: MemoryClaim,
}

impl Download {
    /// Move the exact bytes and their claim into the response that serves them.
    #[cfg(target_os = "windows")]
    pub(crate) fn into_parts(self) -> (Vec<u8>, MemoryClaim) {
        (self.bytes, self.claim)
    }

    /// Borrow the exact staged bytes.
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the bytes charged against the session partition.
    #[cfg(test)]
    pub(crate) fn claimed(&self) -> u64 {
        self.claim.bytes()
    }
}

/// A consumed upload grant whose body is being read outside any registry borrow.
pub(crate) struct UploadSlot {
    token: Token,
    context: i64,
    ticket: Ticket,
}

/// Upload storage that claims capacity before it grows and stops one byte past the bound.
pub(crate) struct UploadBuffer {
    bytes: Vec<u8>,
    claims: Vec<MemoryClaim>,
    claimed: usize,
    lease: MemoryLease,
}

impl UploadBuffer {
    /// Read once into newly claimed, zeroed storage; `Ok(false)` reports the end of the body.
    ///
    /// `read` receives at most [`READ_BYTES`] and returns how many bytes it wrote, or `None`
    /// when the native stream fails.
    pub(crate) fn fill(
        &mut self,
        read: impl FnOnce(&mut [u8]) -> Option<usize>,
    ) -> Result<bool, Refusal> {
        let start = self.bytes.len();
        let want = READ_BYTES.min(MAX_MESSAGE_BYTES + 1 - start);
        if want == 0 {
            return Err(Refusal::Size);
        }
        while self.claimed < start + want {
            let step = UPLOAD_STEP.min(MAX_MESSAGE_BYTES + 1 - self.claimed);
            self.claims.push(self.lease.claim(step as u64).map_err(Refusal::Memory)?);
            self.claimed += step;
            self.bytes.try_reserve_exact(self.claimed - start).map_err(|_| Refusal::Size)?;
        }
        self.bytes.resize(start + want, 0);
        let read = read(&mut self.bytes[start..]).filter(|read| *read <= want);
        let Some(read) = read else {
            self.bytes.truncate(start);
            return Err(Refusal::Read);
        };
        self.bytes.truncate(start + read);
        if self.bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Refusal::Size);
        }
        Ok(read != 0)
    }
}

struct Grant {
    token: Token,
    context: i64,
    ticket: Ticket,
}

struct Received {
    token: Token,
    context: i64,
    ticket: Ticket,
    expires: Instant,
    upload: Upload,
}

struct Staged {
    token: Token,
    context: i64,
    expires: Instant,
    download: Download,
}

#[derive(Default)]
struct State {
    context: Option<i64>,
    closed: bool,
    /// The one live ordinary credit's upload capability.
    grant: Option<Grant>,
    /// The one received upload awaiting its lane envelope.
    received: Option<Received>,
    staged: Vec<Staged>,
}

impl State {
    /// Release every token and body; the window or document no longer owns them.
    fn close(&mut self) {
        self.closed = true;
        self.grant = None;
        self.received = None;
        self.staged.clear();
    }

    /// Release bodies whose single use did not arrive within [`LIFETIME`].
    fn purge(&mut self, now: Instant) {
        if self.received.as_ref().is_some_and(|received| received.expires <= now) {
            self.received = None;
        }
        self.staged.retain(|staged| staged.expires > now);
    }

    fn current(&self, context: i64) -> bool {
        !self.closed && self.context == Some(context)
    }
}

/// Own every bulk token and body for one main lifetime's current panel document.
pub(crate) struct Transfers {
    origin: String,
    lease: MemoryLease,
    random: Box<dyn Fn() -> Result<[u8; 16], DesktopError>>,
    state: RefCell<State>,
}

impl Transfers {
    /// Bind the registry to the page origin allowed to fetch and to its byte partition.
    ///
    /// `origin` is the exact serialized page origin (`http://127.0.0.1:8188`); `lease` is the
    /// dedicated partition bounding in-flight bytes; `random` returns 16 cryptographic bytes.
    pub(crate) fn new(
        origin: String,
        lease: MemoryLease,
        random: impl Fn() -> Result<[u8; 16], DesktopError> + 'static,
    ) -> Result<Self, DesktopError> {
        let mut state = State::default();
        state
            .staged
            .try_reserve_exact(MAX_DOWNLOADS)
            .map_err(|cause| DesktopError::new("allocate_bulk_transfers", cause))?;
        Ok(Self { origin, lease, random: Box::new(random), state: RefCell::new(state) })
    }

    /// Borrow the page origin echoed in CORS responses.
    #[cfg(target_os = "windows")]
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    /// Fence a changed document exactly as panel admission does: old tokens never cross it.
    pub(crate) fn synchronize(&self, context: Option<i64>) {
        let mut state = self.state.borrow_mut();
        if state.context.is_some() && state.context != context {
            state.close();
        }
        if state.context.is_none() && !state.closed {
            state.context = context;
        }
    }

    /// Release every token and body irreversibly while the window retires.
    pub(crate) fn close(&self) {
        self.state.borrow_mut().close();
    }

    /// Issue the upload capability carried with one ordinary credit grant.
    ///
    /// A new grant supersedes the previous credit's capability and any unclaimed body: the
    /// ordinary lane admits one message at a time, so the earlier ticket was already settled.
    ///
    /// # Errors
    /// Rejects a stale document and preserves random generator failures.
    pub(crate) fn grant(&self, context: i64, ticket: Ticket) -> Result<Token, DesktopError> {
        let token = Token::new((self.random)()?);
        let mut state = self.state.borrow_mut();
        if !state.current(context) {
            return Err(DesktopError::message(
                "grant_bulk_upload",
                "panel document is not current",
            ));
        }
        state.received = None;
        state.grant = Some(Grant { token, context, ticket });
        Ok(token)
    }

    /// Match one native request to a live operation without consuming anything.
    ///
    /// The method, exact URL and CORS `Origin` must all match; there is no query, fragment,
    /// alternate spelling or redirect.
    pub(crate) fn route(&self, method: &str, uri: &str, origin: Option<&str>) -> Option<Route> {
        if origin != Some(self.origin.as_str()) {
            return None;
        }
        let path = uri.strip_prefix(ORIGIN)?;
        match (method, path.split_at_checked(2)?) {
            ("GET", ("d/", token)) => Token::parse(token).map(Route::Download),
            ("POST", ("u/", token)) => Token::parse(token).map(Route::Upload),
            _ => None,
        }
    }

    /// Consume a staged message for its one native response.
    pub(crate) fn take_download(&self, token: Token, now: Instant) -> Option<Download> {
        let mut state = self.state.borrow_mut();
        state.purge(now);
        let index = state.staged.iter().position(|staged| staged.token == token)?;
        let staged = state.staged.swap_remove(index);
        state.current(staged.context).then_some(staged.download)
    }

    /// Consume the upload capability before reading any request body.
    pub(crate) fn begin_upload(&self, token: Token) -> Option<UploadSlot> {
        let mut state = self.state.borrow_mut();
        if state.grant.as_ref().is_none_or(|grant| grant.token != token) {
            return None;
        }
        let grant = state.grant.take()?;
        state.current(grant.context).then_some(UploadSlot {
            token: grant.token,
            context: grant.context,
            ticket: grant.ticket,
        })
    }

    /// Lend storage claimed from the session partition for one request body.
    pub(crate) fn upload_buffer(&self) -> UploadBuffer {
        UploadBuffer {
            bytes: Vec::new(),
            claims: Vec::new(),
            claimed: 0,
            lease: self.lease.clone(),
        }
    }

    /// Retain a complete UTF-8 body until its lane envelope arrives.
    ///
    /// # Errors
    /// Refuses a stale document or a non-UTF-8 body; nothing is retained either way.
    pub(crate) fn finish_upload(
        &self,
        slot: UploadSlot,
        buffer: UploadBuffer,
        now: Instant,
    ) -> Result<(), Refusal> {
        let UploadBuffer { bytes, claims, .. } = buffer;
        if bytes.is_empty() {
            return Err(Refusal::Size);
        }
        let text = String::from_utf8(bytes).map_err(|_| Refusal::Size)?;
        let mut state = self.state.borrow_mut();
        if !state.current(slot.context) {
            return Err(Refusal::Closed);
        }
        state.received = Some(Received {
            token: slot.token,
            context: slot.context,
            ticket: slot.ticket,
            expires: now + LIFETIME,
            upload: Upload { text, _claims: claims },
        });
        Ok(())
    }

    /// Consume the upload named by an ordinary lane envelope.
    ///
    /// The token is spent whether or not the rest matches, so a refused envelope cannot be
    /// replayed. The body must belong to this document and ticket, and have the exact size.
    pub(crate) fn take_upload(
        &self,
        context: i64,
        ticket: Ticket,
        envelope: Envelope,
        now: Instant,
    ) -> Option<Upload> {
        let mut state = self.state.borrow_mut();
        state.purge(now);
        if state.grant.as_ref().is_some_and(|grant| grant.token == envelope.token) {
            state.grant = None;
            return None;
        }
        if state.received.as_ref().is_none_or(|received| received.token != envelope.token) {
            return None;
        }
        let received = state.received.take()?;
        (state.current(context)
            && received.context == context
            && received.ticket == ticket
            && received.upload.text.len() == envelope.bytes)
            .then_some(received.upload)
    }

    /// Retain an oversized host message and return the envelope that names it.
    ///
    /// # Errors
    /// Refuses a stale document, an out-of-bounds body, exhausted retention or memory.
    pub(crate) fn stage(
        &self,
        context: i64,
        bytes: Vec<u8>,
        now: Instant,
    ) -> Result<Vec<u8>, Refusal> {
        if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Refusal::Size);
        }
        let token = Token::new((self.random)().map_err(Refusal::Native)?);
        let mut state = self.state.borrow_mut();
        if !state.current(context) {
            return Err(Refusal::Closed);
        }
        state.purge(now);
        if state.staged.len() == MAX_DOWNLOADS {
            return Err(Refusal::Capacity);
        }
        let claim = self.lease.claim(bytes.len() as u64).map_err(Refusal::Memory)?;
        let envelope = Envelope { token, bytes: bytes.len() }.encode().map_err(Refusal::Native)?;
        state.staged.push(Staged {
            token,
            context,
            expires: now + LIFETIME,
            download: Download { bytes, claim },
        });
        Ok(envelope)
    }
}

#[cfg(test)]
#[path = "main_bulk_tests.rs"]
mod tests;
