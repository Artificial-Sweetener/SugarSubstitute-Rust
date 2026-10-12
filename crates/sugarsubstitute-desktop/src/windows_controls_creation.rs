// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt native control slots before interpreting later construction or retirement failures.

use crate::DesktopError;

/// Perform external native effects against the transaction's already retained slots.
pub(super) trait NativePort {
    /// Retained parent registration type supplied by the actual native boundary.
    type Parent;
    /// Created child HWND and its retained native registration.
    type Peer;
    /// Allocate an inert exact parent registration before native installation.
    fn parent(&mut self) -> Result<Self::Parent, DesktopError>;
    /// Install the registration already retained by the transaction.
    fn install_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError>;
    /// Return one created child for immediate adoption before callback setup.
    fn peer(&mut self, index: usize) -> Result<Self::Peer, DesktopError>;
    /// Bind the retained child to its exact typed callback action.
    fn register_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError>;
    /// Install the retained child registration after callback binding.
    fn install_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError>;
    /// Observe completed native child/parent destruction without requesting cleanup.
    fn destroyed(&self, peer: &Self::Peer, parent: Option<&Self::Parent>) -> bool;
    /// Remove the exact child callback before any child HWND destruction.
    fn remove_peer(&mut self, peer: &mut Self::Peer) -> Result<(), DesktopError>;
    /// Destroy a retained HWND only after actual callback removal succeeded.
    fn destroy_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError>;
    /// Retire the exact parent registration after all children have retired.
    fn retire_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError>;
    /// Observe actual returned native callback references independently of empty slots.
    fn callbacks_retired(&self) -> bool;
}

/// Keep callback retirement separate from HWND retirement after a failed destroy.
pub(super) struct PeerSlot<C> {
    pub(super) value: C,
    callbacks_removed: bool,
}

/// Own one initialization attempt and its exact partial native inventory.
pub(super) struct Creation<P, C> {
    pub(super) parent: Option<P>,
    pub(super) peers: Vec<PeerSlot<C>>,
    count: usize,
    attempted: bool,
    complete: bool,
    closing: bool,
}

impl<P, C> Creation<P, C> {
    /// Allocate bounded inert slots before any native admission.
    pub(super) fn new(count: usize) -> Result<Self, DesktopError> {
        let mut peers = Vec::new();
        peers
            .try_reserve_exact(count)
            .map_err(|cause| DesktopError::new("allocate_accessible_peers", cause))?;
        Ok(Self { parent: None, peers, count, attempted: false, complete: false, closing: false })
    }

    /// Fence reentry before effects and retain every created slot before its next fallible step.
    pub(super) fn initialize(
        &mut self,
        port: &mut impl NativePort<Parent = P, Peer = C>,
    ) -> Result<(), DesktopError> {
        if self.attempted || self.closing {
            return Err(DesktopError::message(
                "initialize_accessible_controls",
                "initialization already attempted or closed",
            ));
        }
        self.attempted = true;
        self.parent = Some(port.parent()?);
        let parent = self.parent.as_ref().ok_or_else(|| {
            DesktopError::message("initialize_accessible_parent", "retained parent absent")
        })?;
        port.install_parent(parent)?;
        for index in 0..self.count {
            self.peers.push(PeerSlot { value: port.peer(index)?, callbacks_removed: false });
            let peer = self.peers.last().ok_or_else(|| {
                DesktopError::message("initialize_accessible_peer", "retained peer absent")
            })?;
            port.register_peer(&peer.value)?;
            port.install_peer(&peer.value)?;
        }
        self.complete = true;
        Ok(())
    }

    /// Permit native updates only after this exact construction completed and before closing.
    pub(super) const fn writable(&self) -> bool {
        self.complete && !self.closing
    }

    /// Close monotonically, retaining failed native removals for another actual cleanup turn.
    pub(super) fn shutdown(
        &mut self,
        port: &mut impl NativePort<Parent = P, Peer = C>,
    ) -> Result<(), DesktopError> {
        self.closing = true;
        let mut remaining = Vec::new();
        remaining
            .try_reserve_exact(self.peers.len())
            .map_err(|cause| DesktopError::new("allocate_control_retirement", cause))?;
        let mut failure = None;
        for mut peer in self.peers.drain(..) {
            if port.destroyed(&peer.value, self.parent.as_ref()) {
                continue;
            }
            if !peer.callbacks_removed {
                if let Err(cause) = port.remove_peer(&mut peer.value) {
                    DesktopError::retain_first(&mut failure, cause);
                    remaining.push(peer);
                    continue;
                }
                peer.callbacks_removed = true;
            }
            if let Err(cause) = port.destroy_peer(&peer.value) {
                DesktopError::retain_first(&mut failure, cause);
                remaining.push(peer);
            }
        }
        self.peers = remaining;
        if self.peers.is_empty()
            && let Some(parent) = self.parent.as_ref()
        {
            match port.retire_parent(parent) {
                Ok(()) => self.parent = None,
                Err(cause) => DesktopError::retain_first(&mut failure, cause),
            }
        }
        if !self.retired(port.callbacks_retired()) && self.peers.is_empty() && self.parent.is_none()
        {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message(
                    "release_accessible_callbacks",
                    "native callback references survived completed subclass teardown",
                ),
            );
        }
        match failure {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }

    /// Empty native slots alone cannot establish callback settlement.
    pub(super) fn retired(&self, callbacks_retired: bool) -> bool {
        self.peers.is_empty() && self.parent.is_none() && callbacks_retired
    }

    /// Preserve unresolved native slots only on the defensive unexpected-drop path.
    pub(super) fn retain_unresolved(&mut self) {
        std::mem::forget(std::mem::take(&mut self.peers));
        std::mem::forget(self.parent.take());
    }
}

#[cfg(test)]
#[path = "windows_controls_creation_tests.rs"]
mod tests;
