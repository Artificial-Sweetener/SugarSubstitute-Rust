// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve an observed native predecessor until its own successful release is consumed.
use super::*;
use std::path::Path;

/// Passive ordering barrier issued by an actual previously acquired profile owner.
///
/// The original exact requested path was resolved into this retained account/folder
/// FileID and run under protected directory pins. This barrier authorizes no native
/// effect, treats no path alias as equivalent, and never substitutes for successor
/// acquisition. Only the original owner's consumed successful native release opens it.
pub struct ProfileRetirement {
    shared: Arc<Shared>,
    identity: record::Record,
}
impl ProfileRetirement {
    /// Observe the exact predecessor's completed release, without native calls or waiting.
    pub fn released(&self) -> bool {
        self.shared.release_observed.load(Ordering::Acquire)
    }

    /// Return the observed predecessor run for correlation with authenticated close intent.
    pub fn run(&self) -> RunKey {
        self.identity.run
    }
}
impl WindowsProfileLease {
    /// Issue an ordering barrier for an exact repeat of this owner's original requested path.
    ///
    /// The caller separately verifies authenticated retirement intent. The binding comes
    /// from observed successful acquisition, including native account/folder FileID;
    /// spelling changes and aliases remain subject to ordinary native exclusion checks.
    /// No worker-owned native storage is locked or read on the caller's native thread.
    pub fn successor_retirement(&self, requested: &Path) -> Option<ProfileRetirement> {
        if requested != self.path || self.shared.release_observed.load(Ordering::Acquire) {
            return None;
        }
        let identity = self.observed_identity?;
        if identity.run != self.run {
            return None;
        }
        Some(ProfileRetirement { shared: self.shared.clone(), identity })
    }
}
