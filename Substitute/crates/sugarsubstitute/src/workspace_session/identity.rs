// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate nonwrapping correlation identities within one independently random webview seed.
use super::*;
use sha2::{Digest, Sha256};
pub(super) struct Identities {
    webview: String,
    next: u64,
}
impl Identities {
    pub(super) fn new(webview: String) -> Self {
        Self { webview, next: 0 }
    }
    pub(super) fn next(&mut self) -> Result<String, StoreError> {
        let next = self.next.checked_add(1).ok_or(SessionError::Stale)?;
        let mut hash = Sha256::new();
        hash.update(b"SugarSubstitute.session-correlation.v1\0");
        hash.update(self.webview.as_bytes());
        hash.update(next.to_le_bytes());
        let mut bytes: [u8; 32] = hash.finalize().into();
        bytes[6] = (bytes[6] & 15) | 0x80;
        bytes[8] = (bytes[8] & 63) | 0x80;
        let hex = files::hex(&bytes[..16]);
        self.next = next;
        Ok(format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        ))
    }
}
