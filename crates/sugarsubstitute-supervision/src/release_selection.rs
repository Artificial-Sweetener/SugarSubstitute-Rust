// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Request current-process release selection without caller-provided trust data.

/// Compose the installed-release selector using its compiled immutable trust anchor.
/// Construction performs no filesystem work and does not grant effect authority.
#[derive(Default)]
pub struct InstalledReleaseOwner;

/// Select the actual current process under a separately admitted capture job.
/// This inert request has no caller path, digest, build or signer fields.
pub struct CaptureSelectionRequest {
    _private: (),
}

impl InstalledReleaseOwner {
    /// Construct the stateless composition owner without allocating or opening files.
    pub const fn new() -> Self {
        Self
    }

    /// Request one current-process selection; only actual Core admission grants effects.
    pub const fn capture_selection(&self) -> CaptureSelectionRequest {
        CaptureSelectionRequest { _private: () }
    }
}
