// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Refuse missing or denied restoration without acknowledging a substitute camera.

use super::*;
use sugarsubstitute_canvas::Navigation;
use sugarsubstitute_image_ops::Extent;

#[test]
fn refused_navigation_outcomes_are_actionable_errors() {
    for outcome in [NavigationCommandOutcome::Denied, NavigationCommandOutcome::NoContent] {
        let failure = require_applied(outcome).err();
        assert!(failure.is_some_and(|cause| !cause.to_string().is_empty()));
    }
}

#[test]
fn restoring_without_loaded_geometry_preserves_the_preceding_camera() -> Result<(), Box<dyn Error>>
{
    let mut viewport = Viewport::new(Extent::new(80, 40)?)?;
    viewport.reframe([-20.0, 30.0], 2.0)?;
    let mut tools = ToolManager::new(Navigation::new(viewport));
    let frame = NavigationCommand::Frame { center: [-3.25, 2.75], span: [0.001; 2] };
    assert!(start_at_selection_camera(&mut tools, None, Some(frame), Duration::ZERO).is_err());
    assert_eq!(tools.navigation().viewport(), viewport);
    Ok(())
}
