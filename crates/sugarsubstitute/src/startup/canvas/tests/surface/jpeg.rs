// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select JPEG finals through the shared output admission and real decoded native pixels.

use super::*;
use viewport::fixture::Fixture;

/// Decoded flat fixture colour `(128, 64, 32)` within JPEG and transfer rounding.
fn flat(pixel: u32) -> bool {
    let [alpha, red, green, blue] = pixel.to_be_bytes();
    alpha == 0xff && red.abs_diff(128) <= 3 && green.abs_diff(64) <= 3 && blue.abs_diff(32) <= 3
}

fn output(filename: &str, storage: &str) -> Value {
    json!({"filename":filename,"subfolder":"","type":storage})
}

#[test]
fn a_jpeg_final_presents_its_decoded_pixels() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.select(3, "flat.jpg")?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"read"}))?;
    let read = fixture.complete(5)?;
    assert_eq!(read["viewport"]["dimensions"], json!({"width":19,"height":17}));
    fixture.settle_pixels(|pixels, width| {
        let center = usize::try_from(20 * width + 40).unwrap_or(usize::MAX);
        pixels.get(center).copied().is_some_and(flat)
    })?;
    fixture.finish()
}

#[test]
fn a_jpeg_comparison_pairs_with_a_png_primary() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.select",
        json!({
            "itemId":"result:wide.png", "file":output("wide.png", "output"),
            "comparison":{"itemId":"result:flat.jpg", "file":output("flat.jpg", "output")}
        }),
    )?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"read"}))?;
    let read = fixture.complete(5)?;
    assert_eq!(read["viewport"]["dimensions"], json!({"width":320,"height":160}));
    assert_eq!(read["viewport"]["secondaryDimensions"], json!({"width":19,"height":17}));
    fixture.finish()
}

/// The decoder's explicit rejection reaches the selection; a supported final then recovers.
#[test]
fn an_unsupported_progressive_jpeg_final_replies_error() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.select(3, "progressive.jpg")?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "error")?;
    assert!(fixture.canvas.session.as_ref().is_some_and(Session::source_failed));
    fixture.select(4, "flat.jpg")?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 4, "applied")?;
    assert!(!fixture.canvas.session.as_ref().is_some_and(Session::source_failed));
    fixture.finish()
}

#[test]
fn finals_outside_png_and_jpeg_result_storage_are_refused() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    for (id, file) in [
        (3, output("flat.webp", "output")),
        (4, output("flat", "output")),
        (5, output("flat.jpg", "input")),
        (6, output("flat.webp", "temp")),
    ] {
        fixture.send(id, "surface.select", json!({"itemId":"result:refused", "file":file}))?;
        fixture.take(id, "error")?;
    }
    fixture.finish()
}
