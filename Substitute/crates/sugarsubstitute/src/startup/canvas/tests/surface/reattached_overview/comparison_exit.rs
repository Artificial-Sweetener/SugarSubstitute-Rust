// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer an overview on the committed attachment right after a comparison exit, in every
//! worker completion order, the way run 477 opened the Save Image grid.

use super::*;

/// Host conditions around the overview that follows the comparison exit.
#[derive(Clone, Copy, Debug)]
struct Variant {
    /// Acquire the overview through a host adapter retaining encoded originals in cache custody.
    acquired: bool,
    /// Show the same overview cells twice, so the second overview equals the current scene.
    repeated: bool,
}

/// Like [`await_reply`], but acknowledge every pending frame: a committed attachment answers
/// its selection only once the exact selected frame reached the native tree.
fn await_presented_reply(
    fixture: &mut Fixture,
    gates: &mut Gates,
    id: u64,
    status: &str,
) -> Result<(), Box<dyn Error>> {
    let canvas = ScopeId::new(CANVAS)?;
    loop {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(command) = fixture.canvas.take_command() {
            return check_reply(command, id, status);
        }
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
            continue;
        }
        let work = fixture.scope_work(canvas)?;
        if work.running > gates.blocking() || (work.queued > 0 && !gates.holding()) {
            fixture.wake.wait_after(observed);
        } else if gates.open_next() {
        } else if let Some(deadline) = fixture.canvas.next_deadline() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            fixture.advance(u64::try_from(remaining.as_millis())?.saturating_add(1));
        } else if work.undelivered > 0 {
            fixture.wake.wait_after(observed);
        } else {
            return Err(format!("Request {id} stalled without work, wake or deadline").into());
        }
    }
}

fn read(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    fixture.viewport(id, json!({"type":"read"}))?;
    fixture.complete(id).map(|_| ())
}

fn overview(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    let cell = |filename: &str, x: f64| {
        json!({"itemId":format!("result:{filename}"),
            "file":{"filename":filename,"subfolder":"","type":"output"},
            "rect":{"x":x,"y":0.0,"width":40.0,"height":40.0}})
    };
    fixture.send(
        id,
        "surface.overview",
        json!({"cells":[cell("wide.png", 0.0), cell("green.png", 40.0)]}),
    )
}

/// Compare, leave the comparison, close a menu over the viewport, then open the overview grid.
fn overview_after_comparison_exit(
    order: [Stage; 3],
    variant: Variant,
) -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    read(&mut fixture, 5)?;
    fixture.send(
        6,
        "surface.select",
        json!({"itemId":"result:wide.png",
            "file":{"filename":"wide.png","subfolder":"","type":"output"},
            "comparison":{"itemId":"result:tall.png",
                "file":{"filename":"tall.png","subfolder":"","type":"output"}}}),
    )?;
    fixture.complete(6)?;
    read(&mut fixture, 7)?;
    fixture.select(8, "wide.png")?;
    fixture.complete(8)?;
    read(&mut fixture, 9)?;
    fixture.geometry(10, 80, 40)?;
    fixture.complete(10)?;
    fixture.cancel(10)?;
    cancel_consumed(&mut fixture.canvas, fixture.identity)?;
    fixture.geometry(11, 80, 40)?;
    fixture.complete(11)?;
    let mut id = 12;
    if variant.repeated {
        if variant.acquired {
            let open = Arc::new(Latch::default());
            open.open();
            fixture.canvas.surface.acquire_overviews_with(acquisition::held(open));
        }
        overview(&mut fixture, id)?;
        fixture.complete(id)?;
        id += 1;
    }
    let held_acquisition = Arc::new(Latch::announcing(fixture.wake.clone()));
    if variant.acquired {
        fixture.canvas.surface.acquire_overviews_with(acquisition::held(held_acquisition.clone()));
    }
    let acquisition = variant.acquired.then_some(&held_acquisition);
    let mut gates = Gates::hold(&fixture, order, acquisition)?;
    overview(&mut fixture, id)?;
    await_presented_reply(&mut fixture, &mut gates, id, "applied")
        .map_err(|cause| format!("{order:?} {variant:?}: {cause}"))?;
    gates.finish(&fixture)?;
    fixture.finish()
}

#[test]
fn an_overview_after_a_comparison_exit_is_applied_in_every_worker_completion_order()
-> Result<(), Box<dyn Error>> {
    for variant in [
        Variant { acquired: false, repeated: false },
        Variant { acquired: true, repeated: false },
        Variant { acquired: false, repeated: true },
        Variant { acquired: true, repeated: true },
    ] {
        for order in ORDERS {
            overview_after_comparison_exit(order, variant)?;
        }
    }
    Ok(())
}
