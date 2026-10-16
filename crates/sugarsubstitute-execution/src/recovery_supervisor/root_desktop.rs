// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a genuine native-authenticated Root desktop bootstrap independently of fresh capture.
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, Moment},
    recovery_supervisor::RecoveryGrantKey,
};
/// This private owner is produced by actual paired-section adoption and Root peer authentication.
/// Decoded Root/run/grant bytes or numeric section slots cannot construct a production loan.
pub(crate) struct RootDesktopBootstrapLoan {
    bytes: [u8; 1024],
    key: RecoveryGrantKey,
    counter: CounterDeadline,
    until: Moment,
}
impl RootDesktopBootstrapLoan {
    pub(crate) const fn key(&self) -> RecoveryGrantKey {
        self.key
    }
    pub(crate) const fn counter(&self) -> CounterDeadline {
        self.counter
    }
    pub(crate) const fn until(&self) -> Moment {
        self.until
    }
    /// Borrow only while its genuine recovery source/claim remains retained and current.
    #[cfg(windows)]
    pub(crate) fn bytes(&self) -> &[u8; 1024] {
        &self.bytes
    }
    /// Native admission has verified both strict sections and the actual Root peer.
    #[cfg(windows)]
    pub(crate) fn from_native_authenticated(
        bytes: [u8; 1024],
        key: RecoveryGrantKey,
        counter: CounterDeadline,
        until: Moment,
        peer: crate::lifecycle_pipe::VerifiedPeer,
    ) -> Result<Self, crate::lifecycle_pipe::LifecyclePipeFailure> {
        if peer.binding.recovery() != Some(key)
            || peer.pid == 0
            || peer.birth == 0
            || peer.process.is_some()
        {
            return Err(crate::lifecycle_pipe::failure("recovery_root_bootstrap_peer"));
        }
        Ok(Self { bytes, key, counter, until })
    }
    /// Replace only the external inherited-section/kernel boundary in real Core owner tests.
    #[cfg(test)]
    pub(crate) fn fixture(key: RecoveryGrantKey, counter: CounterDeadline) -> Self {
        Self { bytes: [0; 1024], key, counter, until: Moment::from_nanos(30_000_000_000) }
    }
}
impl Drop for RootDesktopBootstrapLoan {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
