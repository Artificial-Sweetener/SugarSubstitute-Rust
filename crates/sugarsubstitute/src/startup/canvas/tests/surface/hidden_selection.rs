// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A selection on a hidden committed attachment defers until it is shown, and every way the
//! page or host ends that wait answers it exactly once.

use super::*;
use boundary::Execution;
use viewport::fixture::Fixture;

fn hidden_geometry() -> Value {
    json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":80,"height":40}, "clip":{"x":10,"y":20,"width":0,"height":0},
        "physicalPixelsPerCssPixel":1, "visible":false, "focused":false
    }})
}

/// Which selection the page sends while the committed attachment is hidden.
#[derive(Clone, Copy, Debug)]
enum Selection {
    Select,
    Overview,
}

impl Selection {
    const ALL: [Self; 2] = [Self::Select, Self::Overview];

    fn send(self, fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Select => fixture.select(id, "tall.png"),
            Self::Overview => super::overview::present_grid(fixture, id),
        }
    }
}

/// Commit a selection, hide the attachment, then send `selection` as request 6 and run every
/// admitted worker to completion, proving the selection still has no reply.
fn deferred(selection: Selection) -> Result<Fixture, Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.send(5, "surface.geometry", hidden_geometry())?;
    let identity = fixture.identity;
    fixture.canvas.hidden(identity)?;
    fixture.take(5, "applied")?;
    selection.send(&mut fixture, 6)?;
    loop {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        assert!(fixture.canvas.take_command().is_none(), "{selection:?} replied while hidden");
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => return Ok(fixture),
            Execution::Idle => fixture.advance(100),
            Execution::Running | Execution::Delivering => fixture.wake.wait_after(observed),
        }
    }
}

#[test]
fn a_selection_deferred_while_hidden_is_applied_once_the_attachment_is_shown()
-> Result<(), Box<dyn Error>> {
    for selection in Selection::ALL {
        let mut fixture = deferred(selection)?;
        fixture.geometry(7, 80, 40)?;
        fixture.take(7, "applied")?;
        fixture.complete(6)?;
        if matches!(selection, Selection::Select) {
            let shown = fixture.accepted()?.extent;
            assert_eq!(shown, sugarsubstitute_image_ops::Extent::new(160, 320)?);
        } else {
            fixture.settle_pixels(super::overview::grid_pixels)?;
        }
        fixture.finish()?;
    }
    Ok(())
}

#[test]
fn a_selection_deferred_while_hidden_is_cancelled_by_the_page() -> Result<(), Box<dyn Error>> {
    for selection in Selection::ALL {
        let mut fixture = deferred(selection)?;
        fixture.cancel(6)?;
        fixture.take(6, "cancelled")?;
        cancel_consumed(&mut fixture.canvas, fixture.identity)?;
        fixture.geometry(7, 80, 40)?;
        fixture.complete(7)?;
        fixture.finish()?;
    }
    Ok(())
}

#[test]
fn a_selection_deferred_while_hidden_is_unavailable_once_its_attachment_is_released()
-> Result<(), Box<dyn Error>> {
    for selection in Selection::ALL {
        let mut fixture = deferred(selection)?;
        fixture.send(7, "surface.release", json!({}))?;
        fixture.take(6, "unavailable")?;
        fixture.take(7, "applied")?;
        fixture.finish()?;
    }
    Ok(())
}

#[test]
fn closing_while_a_selection_is_deferred_settles_without_a_reply() -> Result<(), Box<dyn Error>> {
    for selection in Selection::ALL {
        // Closing withdraws the page's delivery lane, so the wait retires with its custody.
        deferred(selection)?.finish()?;
    }
    Ok(())
}
