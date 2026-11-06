// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start each selected output at its selection's camera.
//!
//! The page's inspection owner sends each selection's camera: a frame, or nothing for Fit.
//! That selection decides the camera for both freshly loaded and retained sources. A failed
//! reconstruction preserves the preceding document and the pending inspection.

use super::*;
use sugarsubstitute_canvas::{Document, NavigationCommand, NavigationCommandOutcome};

impl Session {
    /// Qualify a retained or parked source before replacing the currently shown document.
    #[cfg(feature = "native-composition")]
    pub(super) fn validate_retained_selection_camera(
        &self,
        key: &retention::RetentionKey,
        frame: Option<NavigationCommand>,
    ) -> Result<(), Box<dyn Error>> {
        let document = if self.document.retention.as_ref() == Some(key) {
            Some(&*self.document)
        } else {
            self.inputs
                .parked_output
                .as_deref()
                .filter(|document| document.retention.as_ref() == Some(key))
                .or_else(|| self.retained.document(key))
        }
        .ok_or("The retained source for the selected camera is unavailable")?;
        let mut navigation = *document.tools.navigation();
        navigation.resize(
            self.document.tools.navigation().viewport().surface(),
            self.navigation_time(),
        )?;
        validate_navigation(
            navigation,
            document.scene.as_ref().map(|scene| &scene.document),
            frame,
            self.navigation_time(),
        )
    }

    /// Borrow the frame offered with this exact selection until its source accepted the camera.
    /// A newer offer replaces older intent; absence means this selection starts at Fit.
    pub(super) fn selection_frame(&self, intent: u64) -> Option<NavigationCommand> {
        self.initial_frame.filter(|(offered, _)| *offered == intent).map(|(_, frame)| frame)
    }

    /// Retire a frame only after its actual source and positive physical extent accepted it.
    pub(super) fn selection_framed(&mut self, intent: u64) {
        if self.initial_frame.is_some_and(|(offered, _)| offered == intent) {
            self.initial_frame = None;
        }
    }
}

/// Validate a source camera through the navigation owner before transferring document custody.
/// `surface` is the current positive physical extent, not an image or CSS extent.
/// # Errors
/// Preserves invalid geometry/transform causes; refuses a frame without actual source content.
pub(super) fn validate_selection_camera(
    surface: sugarsubstitute_image_ops::Extent,
    content: Option<&Document>,
    frame: Option<NavigationCommand>,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    validate_navigation(
        sugarsubstitute_canvas::Navigation::new(Viewport::new(surface)?),
        content,
        frame,
        now,
    )
}

/// Check parameter and navigation-policy admission on a copy, preserving the current camera.
fn validate_navigation(
    mut navigation: sugarsubstitute_canvas::Navigation,
    content: Option<&Document>,
    frame: Option<NavigationCommand>,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    navigation.set_content(content.map(|document| document.snapshot().extent()), now)?;
    if let Some(frame) = frame {
        require_applied(navigation.command(frame, now)?)?;
    }
    Ok(())
}

/// Start a newly loaded output's navigation at its selection's camera: Fit on the current
/// physical extent, then `frame` when one was offered. Validate before changing tools/content.
///
/// # Errors
/// Preserves navigation time and geometry failures.
pub(super) fn start_at_selection_camera(
    tools: &mut ToolManager,
    content: Option<&Document>,
    frame: Option<NavigationCommand>,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    validate_navigation(*tools.navigation(), content, frame, now)?;
    tools.set_content(content.map(|document| document.snapshot().extent()), content, now)?;
    if let Some(frame) = frame {
        apply_selection_frame(tools, frame, now)?;
    }
    Ok(())
}

/// Return a retained output to its selection's camera: `frame`, which places the whole camera,
/// or Fit when none was offered. The camera the document kept while parked is discarded, so it
/// agrees with every other output; a camera that already matches keeps its finished frame.
///
/// # Errors
/// Preserves navigation time and geometry failures.
#[cfg(feature = "native-composition")]
pub(super) fn return_to_selection_camera(
    tools: &mut ToolManager,
    content: Option<&Document>,
    frame: Option<NavigationCommand>,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    match frame {
        Some(frame) => apply_selection_frame(tools, frame, now),
        None if tools.navigation().zoom_mode() != sugarsubstitute_canvas::ZoomMode::Fit => {
            start_at_selection_camera(tools, content, None, now)
        }
        None => Ok(()),
    }
}

/// Apply only the admitted camera; denied or missing content never acknowledges a substitute.
fn apply_selection_frame(
    tools: &mut ToolManager,
    frame: NavigationCommand,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    let outcome = tools.navigate(frame, now)?;
    require_applied(outcome)
}

/// Make camera refusal a failed restoration instead of acknowledging another camera.
fn require_applied(outcome: NavigationCommandOutcome) -> Result<(), Box<dyn Error>> {
    match outcome {
        NavigationCommandOutcome::Applied { .. } => Ok(()),
        NavigationCommandOutcome::Denied => {
            Err("The selected source camera is currently owned by another operation".into())
        }
        NavigationCommandOutcome::NoContent => {
            Err("The selected source camera has no loaded image geometry".into())
        }
    }
}

#[cfg(test)]
mod tests;
