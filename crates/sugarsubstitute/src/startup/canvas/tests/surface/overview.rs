// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present page-laid-out overview cells as one native composition and leave it by selection.

use super::*;
use viewport::fixture::Fixture;

const RED: u32 = 0xffff_0000;
const BLUE: u32 = 0xff00_00ff;

fn cell(filename: &str, rect: [f64; 4]) -> Value {
    json!({
        "itemId":format!("result:{filename}"),
        "file":{"filename":filename,"subfolder":"","type":"output"},
        "rect":{"x":rect[0],"y":rect[1],"width":rect[2],"height":rect[3]}
    })
}

/// Show the 320x160 red and 160x320 blue outputs side by side in two 40x40 CSS cells.
pub(super) fn present_grid(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    fixture.send(
        id,
        "surface.overview",
        json!({"cells":[
            cell("wide.png", [0.0, 0.0, 40.0, 40.0]),
            cell("tall.png", [40.0, 0.0, 40.0, 40.0])
        ]}),
    )
}

/// Each output is fitted inside its cell: red fills 40x20 at rows 10..30, blue 20x40 at
/// columns 50..70; everything else is the matte.
pub(super) fn grid_pixels(pixels: &[u32], width: u32) -> bool {
    pixels.iter().enumerate().all(|(index, &pixel)| {
        let (x, y) = (index as u32 % width, index as u32 / width);
        if x < 40 && (10..30).contains(&y) {
            pixel == RED
        } else if (50..70).contains(&x) {
            pixel == BLUE
        } else {
            pixel != RED && pixel != BLUE
        }
    })
}

#[test]
fn an_overview_fits_every_cell_inside_its_rect_and_locks_the_camera() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::empty()?;
    present_grid(&mut fixture, 3)?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    fixture.settle_pixels(grid_pixels)?;
    fixture.viewport(5, json!({"type":"zoom","factor":2}))?;
    fixture.take(5, "unavailable")?;
    fixture.finish()
}

#[test]
fn selecting_one_result_leaves_the_overview_and_restores_navigation() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::empty()?;
    present_grid(&mut fixture, 3)?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    fixture.settle_pixels(grid_pixels)?;
    fixture.select(5, "tall.png")?;
    fixture.complete(5)?;
    fixture.settle_pixels(|pixels, width| {
        pixels.iter().enumerate().any(|(index, &pixel)| index as u32 % width == 40 && pixel == BLUE)
            && !pixels.contains(&RED)
    })?;
    fixture.viewport(6, json!({"type":"read"}))?;
    let read = fixture.complete(6)?;
    assert_eq!(read["viewport"]["dimensions"], json!({"width":160,"height":320}));
    fixture.finish()
}

#[test]
fn an_overview_records_each_lifecycle_step_once_through_its_reply() -> Result<(), Box<dyn Error>> {
    use crate::canvas_surface_diagnostic::{Operation, Stage, Status, Wait};
    let mut fixture = Fixture::empty()?;
    let lines = DiagnosticLines::default();
    let writer = lines.clone();
    let guard =
        tracing::subscriber::set_default(crate::diagnostics::fixture_subscriber(move || {
            writer.clone()
        }));
    present_grid(&mut fixture, 3)?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    drop(guard);
    let records = lines.requests(3)?;
    assert!(
        records
            .iter()
            .all(|record| record.operation == Operation::Overview && record.revision == 3)
    );
    let stages: Vec<Stage> = records.iter().map(|record| record.stage).collect();
    assert_eq!(stages[..3], [Stage::Admitted, Stage::Offered, Stage::Pending], "{records:?}");
    assert_eq!(stages.last(), Some(&Stage::Replied), "{records:?}");
    assert_eq!(records.last().map(|record| record.status), Some(Status::Applied));
    let waits: Vec<Wait> = records
        .iter()
        .filter(|record| record.stage == Stage::Waiting)
        .map(|record| record.wait)
        .collect();
    assert!(!waits.is_empty() && waits.windows(2).all(|pair| pair[0] != pair[1]), "{waits:?}");
    assert!(!waits.contains(&Wait::Superseded), "{waits:?}");
    assert!(records.len() <= 10, "lifecycle records are bounded: {records:?}");
    fixture.finish()
}

#[test]
fn an_overview_cell_must_be_a_png_or_jpeg_output() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.overview",
        json!({"cells":[cell("wide.png", [0.0, 0.0, 40.0, 40.0]), cell("tall.webp", [40.0, 0.0, 40.0, 40.0])]}),
    )?;
    fixture.take(3, "error")?;
    fixture.finish()
}

#[test]
fn a_partly_scrolled_cell_keeps_its_full_fit_and_is_clipped_by_the_viewport()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.overview",
        json!({"cells":[cell("wide.png", [-20.0, 0.0, 40.0, 40.0])]}),
    )?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    // The 40x20 fit spans x -20..20; only its right half is inside the viewport.
    fixture.settle_pixels(|pixels, width| {
        pixels.iter().enumerate().all(|(index, &pixel)| {
            let (x, y) = (index as u32 % width, index as u32 / width);
            (pixel == RED) == (x < 20 && (10..30).contains(&y))
        })
    })?;
    fixture.finish()
}

#[test]
fn an_overview_requested_while_the_candidate_is_hidden_presents_once_shown()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.geometry",
        json!({"geometry":{
            "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
            "viewport":{"width":80,"height":40}, "clip":{"x":10,"y":20,"width":0,"height":0},
            "physicalPixelsPerCssPixel":1, "visible":false, "focused":false
        }}),
    )?;
    fixture.take(3, "applied")?;
    present_grid(&mut fixture, 4)?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 4, "applied")?;
    fixture.geometry(5, 80, 40)?;
    fixture.take(5, "applied")?;
    fixture.commit(6)?;
    fixture.settle_pixels(grid_pixels)?;
    fixture.finish()
}
