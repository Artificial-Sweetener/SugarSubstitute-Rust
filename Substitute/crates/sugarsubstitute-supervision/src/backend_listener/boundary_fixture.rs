// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed observed matching facts for external HTTP boundary tests.
//!
//! This nondefault fixture performs no kernel calls and creates no native handle,
//! listener lease, family proof or readiness proof. Its values model an external
//! connected-peer observation; they provide no evidence of real TCP qualification.
use super::BackendPeerIdentity;

/// Return fixed immutable HTTP-boundary peer facts without arbitrary identity input.
#[doc(hidden)]
pub const fn fixed_http_peer() -> BackendPeerIdentity {
    BackendPeerIdentity { process_id: 4242, creation_time_100ns: 123456789 }
}
