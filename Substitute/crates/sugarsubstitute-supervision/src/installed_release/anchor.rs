// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select the immutable reviewed development identity in the explicit local K1 build.

/// Borrow the reviewed literal public data; callers cannot supply a signer.
pub(super) struct Anchor {
    pub(super) key_id: &'static [u8; 32],
    pub(super) public_blob: &'static [u8; 411],
}

/// Return the compiled development anchor; ordinary builds have no trusted identity.
pub(super) fn selected() -> Option<Anchor> {
    #[cfg(feature = "local-k1-symbol-trust")]
    {
        Some(Anchor {
            key_id: &super::local_k1_anchor::KEY_ID,
            public_blob: &super::local_k1_anchor::PUBLIC_BLOB,
        })
    }
    #[cfg(not(feature = "local-k1-symbol-trust"))]
    {
        None
    }
}
