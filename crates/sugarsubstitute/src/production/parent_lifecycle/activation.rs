// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Repeated launches may restore only the current authenticated desktop.
use super::*;
impl ParentLifecycle {
    /// Return no target once teardown or failure has begun; normal successor admission owns that case.
    pub(in crate::production) fn activation_peer(&self) -> Option<VerifiedLifecyclePeer> {
        self.peer.filter(|_| self.accepts_reactivation())
    }

    /// An allocated exit transaction is idle until an actual close is accepted.
    pub(super) fn accepts_reactivation(&self) -> bool {
        !(self.closed
            || self.cleanup.is_some()
            || self.normal_close_request().is_some()
            || self.failure.is_some()
            || !self.registered)
    }
}
