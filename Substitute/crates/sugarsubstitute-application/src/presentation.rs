// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Publish only current-state prepared frames while allowing in-flight animation progress.

use sugarsubstitute_contracts::{
    execution::{Generation, IdentityError, SurfaceId},
    presentation::{FrameShape, PreparedFrame},
};

/// Reject a prepared result without discarding the caller's last accepted frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameRejection {
    /// The surface is hidden and cannot publish presentation work.
    Hidden,
    /// Preparation belongs to another application surface.
    ForeignSurface,
    /// Preparation captured an earlier or unrequested future state revision.
    WrongState,
    /// Device dimensions or logical-to-device scale differ from current state.
    ShapeMismatch,
    /// The sample does not advance the last actually accepted sequence.
    OutOfOrder,
}

/// Report publication admission without taking frame storage or terminal-job credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramePublicationDecision {
    /// Publish this current-state sample and retain its frame with the native owner.
    Accepted,
    /// Keep the existing presentation while observing this rejected result.
    Rejected(FrameRejection),
}

/// Own surface state and the last published animation sequence independently of rendering.
///
/// The caller owns retained frames and acknowledges typed job outcomes separately.
/// Newer requested animation samples do not invalidate already running samples;
/// this gate compares only state and the last actually accepted sequence.
pub struct FramePublicationGate {
    surface: SurfaceId,
    shape: FrameShape,
    state_generation: Generation,
    visible: bool,
    last_sequence: Option<Generation>,
}

impl FramePublicationGate {
    /// Open a surface using explicitly validated shape and application-owned identity.
    pub const fn new(
        surface: SurfaceId,
        shape: FrameShape,
        state_generation: Generation,
        visible: bool,
    ) -> Self {
        Self { surface, shape, state_generation, visible, last_sequence: None }
    }

    /// Return the application surface identity.
    pub const fn surface(&self) -> SurfaceId {
        self.surface
    }
    /// Return the current validated device dimensions and scale.
    pub const fn shape(&self) -> FrameShape {
        self.shape
    }
    /// Return the current revision captured by subsequently submitted preparation.
    pub const fn state_generation(&self) -> Generation {
        self.state_generation
    }
    /// Return whether publication is permitted for this surface.
    pub const fn visible(&self) -> bool {
        self.visible
    }
    /// Return the last accepted sample within the current state, if any.
    pub const fn last_accepted_sequence(&self) -> Option<Generation> {
        self.last_sequence
    }

    /// Advance state before new preparation and invalidate previous accepted samples.
    ///
    /// # Errors
    /// Rejects generation exhaustion before changing any publication state.
    pub fn invalidate_state(&mut self) -> Result<Generation, IdentityError> {
        let generation = self.state_generation.next()?;
        self.state_generation = generation;
        self.last_sequence = None;
        Ok(generation)
    }

    /// Advance state before publishing a changed size/scale; equal shapes are inert.
    ///
    /// # Errors
    /// Rejects generation exhaustion, preserving shape and accepted sequence.
    pub fn resize(&mut self, shape: FrameShape) -> Result<Generation, IdentityError> {
        if shape == self.shape {
            return Ok(self.state_generation);
        }
        let generation = self.invalidate_state()?;
        self.shape = shape;
        Ok(generation)
    }

    /// Advance state before changing visibility; repeated values are inert.
    ///
    /// # Errors
    /// Rejects generation exhaustion, preserving visibility and accepted sequence.
    pub fn set_visible(&mut self, visible: bool) -> Result<Generation, IdentityError> {
        if visible == self.visible {
            return Ok(self.state_generation);
        }
        let generation = self.invalidate_state()?;
        self.visible = visible;
        Ok(generation)
    }

    /// Admit current-state pixels using only constant-time immutable metadata.
    ///
    /// Rejections preserve the accepted sequence and never clone/drop frame storage.
    /// Acceptance records a strictly newer sequence without waiting for the latest
    /// requested sample, so in-flight animation can continue to reach presentation.
    pub fn accept(&mut self, frame: &PreparedFrame) -> FramePublicationDecision {
        let identity = frame.identity();
        let rejection = if !self.visible {
            Some(FrameRejection::Hidden)
        } else if identity.surface != self.surface {
            Some(FrameRejection::ForeignSurface)
        } else if identity.state_generation != self.state_generation {
            Some(FrameRejection::WrongState)
        } else if frame.shape() != self.shape {
            Some(FrameRejection::ShapeMismatch)
        } else if self.last_sequence.is_some_and(|last| identity.sequence <= last) {
            Some(FrameRejection::OutOfOrder)
        } else {
            None
        };
        if let Some(rejection) = rejection {
            return FramePublicationDecision::Rejected(rejection);
        }
        self.last_sequence = Some(identity.sequence);
        FramePublicationDecision::Accepted
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
