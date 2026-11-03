// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Selecting an already shown output again presents its retained frame without acquiring it.

use super::*;
use gate::Gate;
use viewport::fixture::{Execution, Fixture};

#[test]
fn selecting_a_shown_output_again_presents_it_once_without_acquiring_it()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    let wide = fixture.accepted()?;
    fixture.select(5, "tall.png")?;
    fixture.complete(5)?;
    fixture.settle()?;
    assert_ne!(fixture.accepted()?.extent, wide.extent);

    let counting = Gate::new(fixture.output("wide.png"), true, fixture.wake.clone());
    fixture.canvas.surface.gate_fixture("wide.png", counting.clone());
    let previous = fixture.accepted()?.source_revision;
    fixture.select(6, "wide.png")?;
    // The operation itself restores the retained document; no later event-loop wake is needed.
    assert_eq!(
        fixture.canvas.session.as_ref().map(Session::source_loading),
        Some(false),
        "The reselection waited for a later poll"
    );
    let mut presents = 0;
    let mut replied = false;
    for _ in 0..500 {
        if let Some(command) = fixture.canvas.take_command() {
            check_reply(command, 6, "applied")?;
            replied = true;
            break;
        }
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            presents += 1;
            fixture.canvas.presented(target)?;
        } else if fixture.execution()? == Execution::Running {
            fixture.wake.wait_after(observed)?;
        }
    }
    assert!(replied, "The reselection was never applied");
    assert_eq!(presents, 1, "The reselection presented more than its retained frame");
    assert!(!counting.entered(), "The reselected output was acquired again");
    let camera = fixture.accepted()?;
    assert!(camera.source_revision > previous);
    assert_eq!(
        Some(camera.source_revision),
        fixture.canvas.session.as_ref().map(Session::source_intent_revision)
    );
    assert_eq!(camera.extent, wide.extent);
    assert_eq!(camera.viewport, wide.viewport);
    fixture.settle()?;
    fixture.finish()
}
