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
/// acquisition. Its release observation covers only the original native owner;
/// the caller must also observe its independent domain profile settlement.
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
    /// Join independently consumed finite success with actual native and helper retirement.
    /// Both terminal-before-ACK and ACK-before-terminal remain within this same owner.
    pub(super) fn publish_release_observation(&self, state: &storage::NativeState) {
        if self.release_successful
            && self.release_operation.is_none()
            && self.pending.is_none()
            && state.retired
            && state.desktop_custody.is_none()
            && state.custody_release.as_ref().is_none_or(|release| release.ack.is_some())
            && self.shared.active.load(Ordering::Acquire) == RELEASED
        {
            self.shared.release_observed.store(true, Ordering::Release);
        }
    }

    /// Issue an ordering barrier for an exact repeat of this owner's original requested path.
    ///
    /// The caller separately verifies authenticated retirement intent. The binding comes
    /// from observed successful acquisition, including native account/folder FileID;
    /// spelling changes and aliases remain subject to ordinary native exclusion checks.
    /// A completed native release retains this historical binding so an entry arriving
    /// before domain settlement can still correlate both owners. Its release fact grants
    /// no fresh acquisition: the successor always repeats ordinary native validation.
    /// No worker-owned native storage is locked or read on the caller's native thread.
    pub fn successor_retirement(&self, requested: &Path) -> Option<ProfileRetirement> {
        if requested != self.path {
            return None;
        }
        let identity = self.observed_identity?;
        if identity.run != self.run {
            return None;
        }
        Some(ProfileRetirement { shared: self.shared.clone(), identity })
    }
}
