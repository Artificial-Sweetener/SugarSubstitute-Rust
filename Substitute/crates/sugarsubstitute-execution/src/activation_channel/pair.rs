// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit child inheritance and once-only early capture retain private connection identity.
use super::*;
use std::os::windows::io::{BorrowedHandle, FromRawHandle, OwnedHandle};
use windows::Win32::Foundation::HANDLE;

/// Parent-owned connection. Include only `with_child`'s handle in the engine's native handle list.
/// Retain through creation and checked child-handle retirement; the endpoint survives job results.
pub struct PreparedChannel {
    endpoint: ChannelEndpoint,
    child: Handle,
    identity: [u8; 32],
}
impl PreparedChannel {
    /// Prepare a private connected pair on admitted execution with `CHANNEL_RESERVATION_BYTES`.
    /// # Errors
    /// Preserves RNG, native creation, peer verification, memory and admission failures.
    pub fn prepare(context: &JobContext) -> Result<Self, NativeObservationError> {
        admit(context)?;
        let memory =
            Arc::new(context.memory().claim(CHANNEL_RESERVATION_BYTES).map_err(|cause| {
                NativeObservationError::new("activation_channel_memory", cause)
            })?);
        let (pipe, child, identity) = native::pair(memory.clone())?;
        Ok(Self { endpoint: ChannelEndpoint::new(pipe, memory, context), child, identity })
    }
    /// Bind broker requests and replies to this original channel's unpredictable identity.
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    /// Retain the parent's endpoint independently of engine creation or worker result delivery.
    pub fn endpoint(&self) -> ChannelEndpoint {
        self.endpoint.clone()
    }
    /// Borrow the exact child endpoint only while this native creator retains the pair.
    /// # Errors
    /// Rejects access after checked retirement; preserves the creation callback's error unchanged.
    pub fn with_child<T>(
        &self,
        body: impl FnOnce(BorrowedHandle<'_>) -> Result<T, NativeObservationError>,
    ) -> Result<T, NativeObservationError> {
        if !self.child.is_armed() {
            return Err(invalid("activation_channel_child_closed"));
        }
        // SAFETY: this pair retains the exclusively owned handle through the nonescaping borrow.
        body(unsafe { BorrowedHandle::borrow_raw(self.child.raw().0) })
    }
    /// Close this parent's child-end duplicate after native engine creation or failed creation.
    /// # Errors
    /// Retains a failed native close for retry; only the inherited engine copy may remain afterward.
    pub fn retire_child(&mut self) -> Result<(), NativeObservationError> {
        self.child.close()
    }
}

/// Initial-entry custody, captured before local handle-owning resources and execution exist.
pub struct InheritedChannel(OwnedHandle);
impl InheritedChannel {
    /// Adopt the captured endpoint into admitted I/O ownership before issuing any operation.
    /// # Errors
    /// Rejects expired/cancelled execution and insufficient `CHANNEL_RESERVATION_BYTES` admission.
    pub fn admit(self, context: &JobContext) -> Result<ChannelEndpoint, NativeObservationError> {
        use std::os::windows::io::IntoRawHandle;
        admit(context)?;
        let memory =
            Arc::new(context.memory().claim(CHANNEL_RESERVATION_BYTES).map_err(|cause| {
                NativeObservationError::new("activation_channel_memory", cause)
            })?);
        let handle = Handle::new(HANDLE(self.0.into_raw_handle()), memory.clone());
        Ok(ChannelEndpoint::new(handle, memory, context))
    }
}

/// Capture only an inheritable connected asynchronous client pipe, then narrow its inheritance.
/// This supplies transport custody, not launch authority; the engine still authenticates its image.
/// # Safety
/// Production callers capture once at initial executable entry, before other local resources.
/// The slot must be uniquely unowned and protected against concurrent close/reuse through this call.
/// Successful capture consumes the slot; no other native or Rust owner may subsequently adopt it.
/// # Errors
/// Rejects invalid, non-inheritable, protected, non-pipe, server or synchronous handles.
pub unsafe fn capture_inherited(raw: usize) -> Result<InheritedChannel, NativeObservationError> {
    native::capture(raw)?;
    // SAFETY: the initial-entry caller supplies exclusive once-only ownership after validation.
    Ok(InheritedChannel(unsafe { OwnedHandle::from_raw_handle(raw as *mut _) }))
}
