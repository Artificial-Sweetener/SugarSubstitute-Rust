// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present temporary Comfy outputs through the same native selection and geometry owners.

use super::*;
use viewport::fixture::Fixture;

/// Temporary PNG/JPEG outputs remain navigable when switching from a saved output and resizing.
#[test]
fn temporary_outputs_present_after_source_switch_and_resize() -> Result<(), Box<dyn Error>> {
    for (filename, width, height) in [("tall.png", 160, 320), ("flat.jpg", 19, 17)] {
        let mut fixture = Fixture::empty()?;
        fixture.select(3, "wide.png")?;
        fixture.complete(3)?;
        fixture.commit(4)?;
        fixture.send(
            5,
            "surface.select",
            json!({"itemId":"result:temporary", "file":{
                "filename":filename,"subfolder":"","type":"temp"
            }}),
        )?;
        fixture.complete(5)?;
        fixture.geometry(6, 140, 100)?;
        fixture.complete(6)?;
        fixture.viewport(7, json!({"type":"read"}))?;
        let read = fixture.complete(7)?;
        assert_eq!(read["viewport"]["dimensions"], json!({"width":width,"height":height}));
        fixture.select(8, "wide.png")?;
        fixture.complete(8)?;
        fixture.finish()?;
    }
    Ok(())
}

/// Comparison sources use the same temporary-output admission as ordinary selections.
#[test]
fn temporary_outputs_can_be_compared_with_saved_outputs() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.select",
        json!({
            "itemId":"result:saved", "file":{"filename":"wide.png","subfolder":"","type":"output"},
            "comparison":{"itemId":"result:temporary","file":{"filename":"flat.jpg","subfolder":"","type":"temp"}}
        }),
    )?;
    fixture.complete(3)?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"read"}))?;
    let read = fixture.complete(5)?;
    assert_eq!(read["viewport"]["secondaryDimensions"], json!({"width":19,"height":17}));
    fixture.finish()
}

/// All-overview cells may show temporary PreviewImage outputs without becoming errors.
#[test]
fn temporary_outputs_fill_native_overview_cells() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.overview",
        json!({"cells":[
            {"itemId":"result:saved", "file":{"filename":"wide.png","subfolder":"","type":"output"},
             "rect":{"x":0,"y":0,"width":40,"height":40}},
            {"itemId":"result:temporary", "file":{"filename":"tall.png","subfolder":"","type":"temp"},
             "rect":{"x":40,"y":0,"width":40,"height":40}}
        ]}),
    )?;
    fixture.complete(3)?;
    fixture.commit(4)?;
    fixture.settle_pixels(overview::grid_pixels)?;
    fixture.finish()
}
