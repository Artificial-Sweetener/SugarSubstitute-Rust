// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry native canvas protocol messages with their navigation and memory custody.
//!
//! A request arrives as a [`CanvasNativeObservation`] holding its ordinary-lane credit. The
//! application answers it with exactly one [`CanvasNativeReply`] carrying that credit back,
//! and sends unsolicited state as [`CanvasNativeNotification`]s, which own no lane.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    canvas_native::{
        CanvasNativeNotificationKind, CanvasNativeNotificationPayload, CanvasNativeReplyPayload,
        CanvasNativeRequest,
    },
    execution::{MemoryClaim, MemoryLease},
    panel::{PanelError, PanelSession},
    startup::MainIdentity,
};

/// A bounded native canvas request attributed to the current native top document.
pub struct CanvasNativeObservation {
    identity: MainIdentity,
    request: CanvasNativeRequest,
    lease: MemoryLease,
    claim: MemoryClaim,
    storage: CanvasNativeRequestStorage,
    hosting: crate::MainHosting,
}

/// Bulk-upload storage that still covers a request decoded from an oversized body.
///
/// Inline requests carry none. Whoever retains the request retains this with it, and drops
/// both together once the request is consumed.
#[derive(Default)]
pub struct CanvasNativeRequestStorage(Vec<MemoryClaim>);

impl CanvasNativeRequestStorage {
    /// Return the bytes this storage holds.
    pub fn bytes(&self) -> u64 {
        self.0.iter().map(MemoryClaim::bytes).sum()
    }
}

impl CanvasNativeObservation {
    /// Retain the attributed request, its credit, any upload storage and the lifetime's hosting.
    #[cfg(target_os = "windows")]
    pub(crate) fn new(
        identity: MainIdentity,
        request: CanvasNativeRequest,
        lease: MemoryLease,
        claim: MemoryClaim,
        upload: Vec<MemoryClaim>,
        hosting: crate::MainHosting,
    ) -> Self {
        Self {
            identity,
            request,
            lease,
            claim,
            storage: CanvasNativeRequestStorage(upload),
            hosting,
        }
    }

    /// Borrow the native navigation that supplied this request.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Return the browser hosting of the main lifetime that supplied this request.
    pub const fn hosting(&self) -> crate::MainHosting {
        self.hosting
    }

    /// Borrow the decoded request.
    pub const fn request(&self) -> &CanvasNativeRequest {
        &self.request
    }

    /// Transfer the request, its lane credit and any upload storage to the application owner.
    ///
    /// The credit returns to the lane only inside the request's single [`CanvasNativeReply`].
    pub fn into_parts(
        self,
    ) -> (MainIdentity, CanvasNativeRequest, MemoryLease, MemoryClaim, CanvasNativeRequestStorage)
    {
        (self.identity, self.request, self.lease, self.claim, self.storage)
    }
}

/// An encoded reply bound to its request's navigation, retaining the request's lane credit.
pub struct CanvasNativeReply {
    identity: MainIdentity,
    #[cfg(any(target_os = "windows", test))]
    request_id: u64,
    payload: Vec<u8>,
    _claim: MemoryClaim,
    _storage: Option<MemoryClaim>,
}

/// Preserve navigation, allocation and identity failures.
#[derive(Debug)]
pub enum CanvasNativeReplyError {
    /// The retained allocation does not cover the message and its native delivery.
    Storage,
    /// The message addresses another main navigation.
    ForeignNavigation,
    /// The current navigation cannot be represented as a panel session.
    Identity(PanelError),
}

impl fmt::Display for CanvasNativeReplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Storage => "Native canvas message storage was not admitted",
            Self::ForeignNavigation => "Native canvas message belongs to another navigation",
            Self::Identity(_) => "Native canvas message navigation identity is invalid",
        })
    }
}

impl Error for CanvasNativeReplyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Identity(cause) => Some(cause),
            Self::Storage | Self::ForeignNavigation => None,
        }
    }
}

/// Require the message to address the panel session of `identity`.
fn validate_session(
    identity: MainIdentity,
    session: &PanelSession,
) -> Result<(), CanvasNativeReplyError> {
    if session != &PanelSession::for_main(identity).map_err(CanvasNativeReplyError::Identity)? {
        return Err(CanvasNativeReplyError::ForeignNavigation);
    }
    Ok(())
}

impl CanvasNativeReply {
    /// Lane credit an ordinary request transfers; replies return exactly this claim.
    pub const STORAGE_BYTES: u64 = 64 * 1024;
    /// Largest payload the lane credit covers; larger replies need [`Self::with_storage`].
    pub const INLINE_PAYLOAD_BYTES: usize = 16 * 1024;

    /// Bind a small encoded reply to its request's navigation and lane credit.
    ///
    /// # Errors
    /// Rejects a credit below [`Self::STORAGE_BYTES`], a payload above
    /// [`Self::INLINE_PAYLOAD_BYTES`], or a reply addressed to another navigation.
    pub fn new(
        identity: MainIdentity,
        payload: CanvasNativeReplyPayload,
        claim: MemoryClaim,
    ) -> Result<Self, CanvasNativeReplyError> {
        if payload.as_bytes().len() > Self::INLINE_PAYLOAD_BYTES {
            return Err(CanvasNativeReplyError::Storage);
        }
        Self::bind(identity, payload, claim, None)
    }

    /// Bind a large encoded reply, such as a capture, with separately admitted storage.
    ///
    /// `storage` covers the payload until delivery stages it for the bulk channel; the lane
    /// credit returns to its lane on completion as usual.
    /// # Errors
    /// Rejects inadequate credit or storage, or a reply addressed to another navigation.
    pub fn with_storage(
        identity: MainIdentity,
        payload: CanvasNativeReplyPayload,
        claim: MemoryClaim,
        storage: MemoryClaim,
    ) -> Result<Self, CanvasNativeReplyError> {
        if storage.bytes() < u64::try_from(payload.as_bytes().len()).unwrap_or(u64::MAX) {
            return Err(CanvasNativeReplyError::Storage);
        }
        Self::bind(identity, payload, claim, Some(storage))
    }

    fn bind(
        identity: MainIdentity,
        payload: CanvasNativeReplyPayload,
        claim: MemoryClaim,
        storage: Option<MemoryClaim>,
    ) -> Result<Self, CanvasNativeReplyError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(CanvasNativeReplyError::Storage);
        }
        validate_session(identity, payload.session())?;
        Ok(Self {
            identity,
            #[cfg(any(target_os = "windows", test))]
            request_id: payload.request_id(),
            payload: payload.into_bytes(),
            _claim: claim,
            _storage: storage,
        })
    }

    /// Borrow the exact native navigation addressed by this reply.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow schema-validated UTF-8 while the original allocation remains retained.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Move oversized encoded bytes to the bulk owner; the claims remain with delivery custody.
    #[cfg(target_os = "windows")]
    pub(crate) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }

    /// Return the original request correlation without decoding the retained payload.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Drop encoded bytes and payload storage before returning the lane credit.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        let Self { payload, _claim, _storage, .. } = self;
        drop((payload, _storage));
        _claim
    }
}

/// One encoded notification for the current navigation; it owns no lane credit.
///
/// An undelivered notification is replaced by a newer one of the same kind, since each
/// carries complete state.
pub struct CanvasNativeNotification {
    identity: MainIdentity,
    kind: CanvasNativeNotificationKind,
    payload: Vec<u8>,
    _claim: MemoryClaim,
}

impl CanvasNativeNotification {
    /// Storage each notification holds through native delivery.
    pub const STORAGE_BYTES: u64 = 64 * 1024;

    /// Bind an encoded notification to the navigation it addresses.
    ///
    /// # Errors
    /// Rejects a claim below [`Self::STORAGE_BYTES`] or the payload, or another navigation.
    pub fn new(
        identity: MainIdentity,
        payload: CanvasNativeNotificationPayload,
        claim: MemoryClaim,
    ) -> Result<Self, CanvasNativeReplyError> {
        let length = u64::try_from(payload.as_bytes().len()).unwrap_or(u64::MAX);
        if claim.bytes() < Self::STORAGE_BYTES.max(length) {
            return Err(CanvasNativeReplyError::Storage);
        }
        validate_session(identity, payload.session())?;
        Ok(Self { identity, kind: payload.kind(), payload: payload.into_bytes(), _claim: claim })
    }

    /// Borrow the native navigation this notification addresses.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Return which notification this is.
    pub const fn kind(&self) -> CanvasNativeNotificationKind {
        self.kind
    }

    /// Borrow schema-validated UTF-8 while its allocation remains retained.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Move oversized encoded bytes to the bulk owner; the claim remains with delivery custody.
    #[cfg(target_os = "windows")]
    pub(crate) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }

    /// Whether `other` supersedes this undelivered notification.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn same_target(&self, other: &Self) -> bool {
        self.identity == other.identity && self.kind == other.kind
    }

    /// Drop encoded bytes before returning the independently admitted claim.
    #[cfg(target_os = "windows")]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        let Self { payload, _claim, .. } = self;
        drop(payload);
        _claim
    }
}

#[cfg(test)]
mod tests;
