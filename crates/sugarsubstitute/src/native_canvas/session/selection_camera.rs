// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start each selected output at its selection's camera.
//!
//! The page's inspection owner holds the one view shared by every output and sends it with each
//! selection: a frame, or nothing for Fit. The selection alone decides the camera, so a freshly
//! loaded output and a retained one start alike, and no document keeps a camera of its own across
//! selections.

use super::*;
use sugarsubstitute_canvas::{Document, NavigationCommand, NavigationCommandOutcome};

impl Session {
    /// Take the frame offered with selection `intent`; a frame offered with another selection
    /// is discarded, and `None` means that selection starts at Fit.
    pub(super) fn selection_frame(&mut self, intent: u64) -> Option<NavigationCommand> {
        self.initial_frame.take().filter(|(offered, _)| *offered == intent).map(|(_, frame)| frame)
    }
}

/// Start a newly loaded output's navigation at its selection's camera: Fit on the current
/// physical extent, then `frame` when one was offered. A frame the navigation owner refuses
/// leaves the output at Fit.
///
/// # Errors
/// Preserves navigation time and geometry failures.
pub(super) fn start_at_selection_camera(
    tools: &mut ToolManager,
    content: Option<&Document>,
    frame: Option<NavigationCommand>,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
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

fn apply_selection_frame(
    tools: &mut ToolManager,
    frame: NavigationCommand,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    let outcome = tools.navigate(frame, now)?;
    if !matches!(outcome, NavigationCommandOutcome::Applied { .. }) {
        tracing::debug!(operation = "canvas_selection_frame", outcome = ?outcome,
            "Selection camera was not admitted; the output keeps its current camera");
    }
    Ok(())
}
