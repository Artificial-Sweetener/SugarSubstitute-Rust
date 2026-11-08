// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute native top-document session input before handing its claimed DTO to the root.
use super::{Channel, queue};
use crate::{DesktopError, main_session_contract::contract_error};
impl Channel {
    pub(super) fn decode(
        &self,
        source: &str,
        payload: &str,
    ) -> Result<Option<crate::main_session_ingress::Received>, DesktopError> {
        crate::main_session_ingress::decode(
            &self.bootstrap,
            self.identity,
            source,
            payload,
            &self.memory,
            &self.lease,
        )
    }
    /// Post only after all native getters/queue borrows have returned on this UI apartment.
    pub(super) fn post(&self, value: &impl serde::Serialize) -> Result<(), DesktopError> {
        if self.events.in_callback() {
            return Err(DesktopError::message("post_session_message", "callback cannot post"));
        }
        let mut source = windows::core::PWSTR::null();
        // SAFETY: UI-affine retained core writes one owned native string before any queue borrow.
        let observed = unsafe { self.core.Source(&mut source) };
        let source = crate::windows_main_receivers::NativeString(source);
        observed.map_err(|cause| DesktopError::new("session_current_source", cause))?;
        if source.0.is_null() {
            return Err(DesktopError::message("session_current_source", "native source missing"));
        }
        let mut text = String::new();
        text.try_reserve_exact(queue::SOURCE_BYTES)
            .map_err(|cause| DesktopError::new("session_current_source", cause))?;
        queue::copy(&mut text, queue::SOURCE_BYTES, |index| {
            // SAFETY: The completed getter's owned NUL-terminated storage remains live here.
            unsafe { source.0.as_ptr().add(index).read() }
        })?;
        if !crate::main_navigation::admits_url(self.bootstrap.origin(), &text) {
            return Err(contract_error(
                "session_current_source",
                sugarsubstitute_contracts::desktop_session::SessionError::UntrustedOrigin,
            ));
        }
        let encoded = serde_json::to_string(value)
            .map_err(|cause| DesktopError::new("encode_session_post", cause))?;
        if encoded.len() > queue::FRAME_BYTES {
            return Err(contract_error(
                "post_session_message",
                sugarsubstitute_contracts::desktop_session::SessionError::TooLarge,
            ));
        }
        let encoded = windows::core::HSTRING::from(encoded);
        // SAFETY: The actor retains its actual UI-affine core; the bounded string lives through this call.
        unsafe { self.core.PostWebMessageAsJson(&encoded) }
            .map_err(|cause| DesktopError::new("post_session_message", cause))
    }
}
