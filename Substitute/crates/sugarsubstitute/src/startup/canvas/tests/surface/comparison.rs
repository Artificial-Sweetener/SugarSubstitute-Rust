// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select a result pair, report both extents and move a viewport-anchored wipe in native pixels.

use super::*;
use viewport::fixture::Fixture;

const RED: u32 = 0xffff_0000;
const BLUE: u32 = 0xff00_00ff;

fn output(filename: &str) -> Value {
    json!({"filename":filename,"subfolder":"","type":"output"})
}

fn select_pair(fixture: &mut Fixture, id: u64, compared: &str) -> Result<(), Box<dyn Error>> {
    fixture.send(
        id,
        "surface.select",
        json!({
            "itemId":"result:wide.png", "file":output("wide.png"),
            "comparison":{"itemId":format!("result:{compared}"), "file":output(compared)}
        }),
    )
}

fn wipe(fixture: &mut Fixture, id: u64, split: f64) -> Result<(), Box<dyn Error>> {
    fixture.viewport(id, json!({"type":"wipe","orientation":"vertical","split":split}))
}

/// Settle until every pixel equals `expected` for its column.
fn settle_pixels(
    fixture: &mut Fixture,
    expected: impl Fn(u32) -> u32,
) -> Result<(), Box<dyn Error>> {
    fixture.settle_pixels(|pixels, width| {
        pixels.iter().enumerate().all(|(index, &pixel)| {
            pixel == expected(u32::try_from(index).unwrap_or(u32::MAX) % width)
        })
    })?;
    Ok(())
}
#[test]
fn a_selected_pair_reports_both_extents_and_wipes_in_native_pixels() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    select_pair(&mut fixture, 3, "tall.png")?;
    fixture.wait_reply(3, "applied")?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"read"}))?;
    let read = fixture.complete(5)?;
    assert_eq!(read["viewport"]["dimensions"], json!({"width":320,"height":160}));
    assert_eq!(read["viewport"]["secondaryDimensions"], json!({"width":160,"height":320}));
    settle_pixels(&mut fixture, |x| if x < 40 { BLUE } else { RED })?;
    wipe(&mut fixture, 6, 0.25)?;
    let moved = fixture.complete(6)?;
    assert_eq!(moved["viewport"]["secondaryDimensions"], json!({"width":160,"height":320}));
    settle_pixels(&mut fixture, |x| if x < 20 { BLUE } else { RED })?;
    wipe(&mut fixture, 7, 1.0)?;
    fixture.complete(7)?;
    settle_pixels(&mut fixture, |_| BLUE)?;
    fixture.finish()
}

#[test]
fn a_single_selection_refuses_wipes_and_reports_no_comparison() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    wipe(&mut fixture, 5, 0.5)?;
    fixture.take(5, "unavailable")?;
    fixture.viewport(6, json!({"type":"read"}))?;
    let read = fixture.complete(6)?;
    assert!(read["viewport"].get("secondaryDimensions").is_none());
    fixture.finish()
}

#[test]
fn a_comparison_must_be_a_png_or_jpeg_output_like_the_primary() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    select_pair(&mut fixture, 3, "tall.webp")?;
    fixture.take(3, "error")?;
    fixture.finish()
}
