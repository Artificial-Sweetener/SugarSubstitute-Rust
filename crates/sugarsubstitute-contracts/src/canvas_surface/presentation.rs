// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode observed presentation changes and correlated session watch outcomes.

use super::{
    CanvasSurfaceAddress, CanvasSurfaceDiscoveryStatus, CanvasSurfaceMessageError,
    CanvasViewportMode, CanvasViewportState, reply::encode, wire,
};
use crate::panel::PanelSession;

/// A validated watch request; its session still requires current-navigation qualification.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfacePresentationRequest {
    session: PanelSession,
    request_id: u64,
    enabled: bool,
}

/// The observed terminal outcome for the preview lease named by a presentation change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfacePresentationStatus {
    /// The preview reached native presentation.
    Presented,
    /// The preview failed; the snapshot reports the image still displayed.
    Failed,
    /// The preview was withdrawn or superseded.
    Cancelled,
}

/// The native owner's observed image, with source-pixel dimensions and physical scale.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasSurfacePresentationDisplay {
    /// The native surface displays no image.
    Empty,
    /// A preview lease is displayed.
    Preview {
        /// Correlation token for the displayed preview.
        token: String,
        /// Original begin revision for the displayed preview.
        lease_revision: u64,
        /// Actual viewport state for the displayed preview; its mode is always Fit.
        viewport: CanvasViewportState,
    },
    /// A selected final result is displayed.
    Final {
        /// The selection operation revision identifying that final result.
        selection_revision: u64,
        /// Actual viewport state for the displayed image.
        viewport: CanvasViewportState,
    },
    /// A composed overview grid is displayed.
    Overview {
        /// The overview operation revision identifying the displayed grid.
        selection_revision: u64,
    },
}

/// A validated immutable change; encoding allocates no request identity and grants no authority.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfacePresentationChange {
    address: CanvasSurfaceAddress,
    sequence: u64,
    token: String,
    lease_revision: u64,
    status: CanvasSurfacePresentationStatus,
    displayed: CanvasSurfacePresentationDisplay,
    message: String,
}

impl CanvasSurfacePresentationRequest {
    /// Validate generated watch data before exposing its correlation identities.
    pub(super) fn from_wire(
        value: wire::CanvasSurfacePresentationRequest,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(Self {
            session: PanelSession::new(value.session)
                .map_err(|_| CanvasSurfaceMessageError::Invalid)?,
            request_id: value.request_id,
            enabled: value.enabled,
        })
    }

    /// Borrow the public session label for independent navigation checks.
    pub fn session(&self) -> &PanelSession {
        &self.session
    }
    /// Return the request identity within this panel session.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
    /// Return whether the caller requests future observed presentation changes.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Encode the observed watch state for this exact session and request.
    ///
    /// `enabled` reports the native owner's current watch state; a failure may retain an
    /// existing watch. Presentation observations grant no native or attachment authority.
    /// # Errors
    /// Rejects oversized text, invalid schema values and serialization failures.
    pub fn reply(
        &self,
        status: CanvasSurfaceDiscoveryStatus,
        enabled: bool,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        let value = wire::CanvasSurfacePresentationReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "surface.presentation.reply".into(),
            schema: 1,
            enabled,
            status: status.as_str().into(),
            message: message.into(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        encode(&value)
    }
}

impl CanvasSurfacePresentationChange {
    /// Validate a native observation before allowing asynchronous wire encoding.
    ///
    /// Sequence and revision identities use JavaScript's exact positive integer range.
    /// The lease identifies the original begin operation even after camera revisions.
    /// Callers qualify the attachment and publish only observations from its native owner.
    /// A presented outcome requires the exact reported preview lease in Fit mode. Every
    /// displayed preview uses Fit, including a retained preview after another lease fails.
    /// # Errors
    /// Rejects invalid identities, revisions, tokens, viewport snapshots or oversized text.
    pub fn new(
        address: CanvasSurfaceAddress,
        sequence: u64,
        token: String,
        lease_revision: u64,
        status: CanvasSurfacePresentationStatus,
        displayed: CanvasSurfacePresentationDisplay,
        message: String,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        let value = Self { address, sequence, token, lease_revision, status, displayed, message };
        if !value.to_wire().valid() || !value.valid_presentation() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(value)
    }

    /// Borrow the qualified observation's complete attachment correlation address.
    pub fn address(&self) -> &CanvasSurfaceAddress {
        &self.address
    }

    /// Return the positive observation sequence used to coalesce pending snapshots.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Encode this observation for an independently qualified watching panel session.
    ///
    /// Asynchronous changes always use request identity zero and presentation schema one.
    /// The envelope retains the shared surface-message byte bound.
    /// # Errors
    /// Rejects invalid session data, oversized output and serialization failures.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        let value = wire::CanvasSurfacePresentationChanged {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: session.as_str().into(),
            request_id: 0,
            r#type: "surface.presentation.changed".into(),
            schema: 1,
            change: self.to_wire(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        encode(&value)
    }

    /// Qualify observed preview policy independently of the generated schema's field bounds.
    fn valid_presentation(&self) -> bool {
        match &self.displayed {
            CanvasSurfacePresentationDisplay::Preview { token, lease_revision, viewport } => {
                viewport.mode() == CanvasViewportMode::Fit
                    && (self.status != CanvasSurfacePresentationStatus::Presented
                        || (token == &self.token && lease_revision == &self.lease_revision))
            }
            CanvasSurfacePresentationDisplay::Empty
            | CanvasSurfacePresentationDisplay::Final { .. }
            | CanvasSurfacePresentationDisplay::Overview { .. } => {
                self.status != CanvasSurfacePresentationStatus::Presented
            }
        }
    }

    /// Translate validated native observations into the generated untrusted wire shape.
    fn to_wire(&self) -> wire::CanvasSurfacePresentationChange {
        wire::CanvasSurfacePresentationChange {
            address: self.address.wire().clone(),
            sequence: self.sequence,
            token: self.token.clone(),
            lease_revision: self.lease_revision,
            status: self.status.as_str().into(),
            displayed: self.displayed.to_wire(),
            message: self.message.clone(),
        }
    }
}

impl CanvasSurfacePresentationDisplay {
    /// Translate the typed observation to the closed generated wire representation.
    fn to_wire(&self) -> wire::CanvasSurfacePresentationDisplay {
        match self {
            Self::Empty => wire::CanvasSurfacePresentationDisplay::Empty {},
            Self::Preview { token, lease_revision, viewport } => {
                wire::CanvasSurfacePresentationDisplay::Preview {
                    token: token.clone(),
                    lease_revision: *lease_revision,
                    viewport: viewport.to_wire(),
                }
            }
            Self::Final { selection_revision, viewport } => {
                wire::CanvasSurfacePresentationDisplay::Final {
                    selection_revision: *selection_revision,
                    viewport: viewport.to_wire(),
                }
            }
            Self::Overview { selection_revision } => {
                wire::CanvasSurfacePresentationDisplay::Overview {
                    selection_revision: *selection_revision,
                }
            }
        }
    }
}

impl CanvasSurfacePresentationStatus {
    /// Return the shared schema's observed outcome token.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Presented => "presented",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[cfg(test)]
mod tests;
