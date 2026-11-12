// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode the settled camera of a host-routed gesture for the watching panel session.

use super::{
    CanvasSurfaceAddress, CanvasSurfaceMessageError, CanvasViewportState, reply::encode, wire,
};
use crate::panel::PanelSession;

/// The settled native camera after a gesture the host routed to its navigation tool.
///
/// One change describes one settled or cancelled gesture on the shown final selection, never an
/// intermediate move. Its sequence shares the presentation observation sequence, so either
/// kind of notification fences older ones. It grants no native authority.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceCameraChange {
    address: CanvasSurfaceAddress,
    sequence: u64,
    selection_revision: u64,
    viewport: CanvasViewportState,
}

impl CanvasSurfaceCameraChange {
    /// Validate a settled camera before allowing asynchronous wire encoding.
    ///
    /// `sequence` and `selection_revision` use JavaScript's exact positive integer range;
    /// `selection_revision` is the revision of the selection whose final image is shown.
    /// # Errors
    /// Rejects zero or out-of-range identities.
    pub fn new(
        address: CanvasSurfaceAddress,
        sequence: u64,
        selection_revision: u64,
        viewport: CanvasViewportState,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        let value = Self { address, sequence, selection_revision, viewport };
        if !value.to_wire().valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(value)
    }

    /// Borrow the attachment correlation address.
    pub fn address(&self) -> &CanvasSurfaceAddress {
        &self.address
    }

    /// Return the positive observation sequence shared with presentation changes.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Return the revision of the shown final selection.
    pub const fn selection_revision(&self) -> u64 {
        self.selection_revision
    }

    /// Return the settled native camera.
    pub const fn viewport(&self) -> CanvasViewportState {
        self.viewport
    }

    /// Encode this camera for an independently qualified watching panel session.
    ///
    /// Asynchronous changes use request identity zero and schema two within the shared
    /// surface-message byte bound.
    /// # Errors
    /// Rejects invalid session data, oversized output and serialization failures.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        let value = wire::CanvasSurfaceCameraChanged {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: session.as_str().into(),
            request_id: 0,
            r#type: "surface.camera.changed".into(),
            schema: 2,
            change: self.to_wire(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        encode(&value)
    }

    fn to_wire(&self) -> wire::CanvasSurfaceCameraChange {
        wire::CanvasSurfaceCameraChange {
            address: self.address.wire().clone(),
            sequence: self.sequence,
            selection_revision: self.selection_revision,
            viewport: self.viewport.to_wire(),
        }
    }
}

#[cfg(test)]
mod tests;
