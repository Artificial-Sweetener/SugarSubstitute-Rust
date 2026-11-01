// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the native navigation tool with desktop-routed wheel and drag gestures.
//!
//! The fixture's viewport sits at CSS (10, 20) with density 1, so its physical client origin
//! is (10, 20) and routed client positions subtract that origin.

use super::*;
use crate::startup::canvas::RoutedInput;
use sugarsubstitute_desktop::canvas::{
    MainCanvasButton, MainCanvasDragOut, MainCanvasGesture, MainCanvasInput,
};
use viewport::fixture::Fixture;

const PRIMARY: MainCanvasButton = MainCanvasButton::Primary;

/// Deliver gestures through the same entry point as the main-loop observation route.
fn route(
    fixture: &mut Fixture,
    gestures: &[MainCanvasGesture],
) -> Result<Vec<RoutedInput>, Box<dyn Error>> {
    gestures
        .iter()
        .map(|gesture| {
            fixture.canvas.input(MainCanvasInput { identity: fixture.identity, gesture: *gesture })
        })
        .collect()
}

fn button(pressed: bool) -> MainCanvasGesture {
    MainCanvasGesture::Button { button: PRIMARY, pressed }
}

/// Read the settled camera through the ordinary viewport contract.
fn read(fixture: &mut Fixture, id: u64) -> Result<Value, Box<dyn Error>> {
    fixture.viewport(id, json!({"type":"read"}))?;
    Ok(fixture.complete(id)?["viewport"].clone())
}

/// Require every gesture to be refused and the camera to stay unrouted.
fn refused(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    assert!(fixture.canvas.input_region().is_none());
    let results =
        route(fixture, &[MainCanvasGesture::Pointer([30.0, 30.0]), MainCanvasGesture::Wheel(1.0)])?;
    assert_eq!(results, [RoutedInput::Refused; 2]);
    Ok(())
}

#[test]
fn the_committed_final_publishes_its_acknowledged_region_and_projected_occlusions()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let region = fixture.canvas.input_region().ok_or("Committed final has no input region")?;
    assert_eq!(region.identity, fixture.identity);
    assert_eq!(region.placement.clip, PhysicalRect { x: 10, y: 20, width: 80, height: 40 });
    assert!(region.occlusions.is_empty());
    let geometry = |extra: Value| {
        let mut geometry = json!({
            "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
            "viewport":{"width":80,"height":40}, "clip":{"x":10,"y":20,"width":80,"height":40},
            "physicalPixelsPerCssPixel":1.0, "visible":true, "focused":true
        });
        if let (Some(base), Some(extra)) = (geometry.as_object_mut(), extra.as_object()) {
            base.extend(extra.clone());
        }
        json!({ "geometry": geometry })
    };
    fixture.send(
        5,
        "surface.geometry",
        geometry(json!({"occlusions":[{"x":60.5,"y":0,"width":19.5,"height":12}]})),
    )?;
    fixture.take(5, "applied")?;
    let region = fixture.canvas.input_region().ok_or("Occluded final has no input region")?;
    assert_eq!(region.occlusions, [PhysicalRect { x: 70, y: 20, width: 20, height: 12 }]);
    fixture.send(6, "surface.geometry", geometry(json!({"modal":true})))?;
    fixture.take(6, "applied")?;
    refused(&mut fixture)?;
    fixture.finish()
}

#[test]
fn a_wheel_notch_zooms_the_committed_final_in_at_the_cursor() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let before = read(&mut fixture, 5)?;
    assert_eq!(before["mode"], "fit");
    let camera = fixture.accepted()?.viewport;
    let cursor = [30.0, 30.0];
    let local = [cursor[0] - 10.0, cursor[1] - 20.0];
    let anchored =
        |origin: [f64; 2], zoom: f64| [0, 1].map(|axis| origin[axis] + local[axis] / zoom);
    let document = anchored(camera.origin(), camera.zoom());
    let results =
        route(&mut fixture, &[MainCanvasGesture::Pointer(cursor), MainCanvasGesture::Wheel(1.0)])?;
    assert_eq!(results, [RoutedInput::Applied; 2]);
    let after = read(&mut fixture, 6)?;
    assert_eq!(after["mode"], "custom");
    let scale = |state: &Value| state["physicalScale"].as_f64().unwrap_or_default();
    assert!(scale(&after) > scale(&before), "{before} -> {after}");
    let camera = fixture.accepted()?.viewport;
    let zoomed = anchored(camera.origin(), camera.zoom());
    for axis in 0..2 {
        assert!((zoomed[axis] - document[axis]).abs() < 1e-6, "{document:?} -> {zoomed:?}");
    }
    fixture.finish()
}

#[test]
fn a_primary_drag_pans_the_zoomed_camera_by_the_dragged_distance() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":4}))?;
    fixture.complete(5)?;
    let before = fixture.accepted()?.viewport.origin();
    route(
        &mut fixture,
        &[
            MainCanvasGesture::Pointer([50.0, 40.0]),
            button(true),
            MainCanvasGesture::Pointer([60.0, 44.0]),
            button(false),
        ],
    )?;
    read(&mut fixture, 6)?;
    let camera = fixture.accepted()?.viewport;
    let after = camera.origin();
    // Dragging right/down by (10, 4) device pixels moves the image with the pointer.
    assert!((before[0] - after[0] - 10.0 / camera.zoom()).abs() < 1e-6, "{before:?} -> {after:?}");
    assert!((before[1] - after[1] - 4.0 / camera.zoom()).abs() < 1e-6, "{before:?} -> {after:?}");
    fixture.finish()
}

#[test]
fn a_click_then_a_keyboard_pan_applies_and_keeps_the_camera_readable() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":4}))?;
    fixture.complete(5)?;
    let results = route(
        &mut fixture,
        &[MainCanvasGesture::Pointer([50.0, 40.0]), button(true), button(false)],
    )?;
    assert_eq!(results, [RoutedInput::Applied; 3]);
    let settled = read(&mut fixture, 6)?;
    assert_eq!(settled["mode"], "custom");
    fixture.viewport(7, json!({"type":"pan","x":0.1,"y":0,"units":"viewport"}))?;
    let panned = fixture.complete(7)?;
    assert!(panned["viewport"]["physicalScale"].as_f64().unwrap_or_default() > 0.0, "{panned}");
    fixture.finish()
}

#[test]
fn a_camera_command_during_a_held_drag_is_refused_until_the_release_lands()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":4}))?;
    fixture.complete(5)?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([50.0, 40.0]), button(true)])?;
    fixture.viewport(6, json!({"type":"pan","x":0.1,"y":0,"units":"viewport"}))?;
    fixture.take(6, "unavailable")?;
    assert_eq!(route(&mut fixture, &[button(false)])?, [RoutedInput::Applied]);
    let settled = read(&mut fixture, 7)?;
    assert_eq!(settled["mode"], "custom");
    fixture.viewport(8, json!({"type":"pan","x":0.1,"y":0,"units":"viewport"}))?;
    fixture.complete(8)?;
    fixture.finish()
}

#[test]
fn a_cancellation_ends_a_held_drag_without_moving_the_camera() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":4}))?;
    fixture.complete(5)?;
    let before = fixture.accepted()?.viewport.origin();
    route(
        &mut fixture,
        &[MainCanvasGesture::Pointer([50.0, 40.0]), button(true), MainCanvasGesture::Cancel],
    )?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([70.0, 50.0])])?;
    read(&mut fixture, 6)?;
    assert_eq!(fixture.accepted()?.viewport.origin(), before);
    fixture.viewport(7, json!({"type":"pan","x":0.1,"y":0,"units":"viewport"}))?;
    fixture.complete(7)?;
    fixture.finish()
}

#[test]
fn routed_input_before_a_committed_final_is_refused() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    refused(&mut fixture)?;
    fixture.finish()
}

#[test]
fn routed_input_during_a_shown_preview_is_refused() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    assert!(fixture.canvas.input_region().is_some());
    preview::watch(&mut fixture, 100, true)?;
    preview::transfer(&mut fixture, 200, "preview:1", preview::JPEG)?;
    let target = preview::wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    preview::wait_change(&mut fixture, "presented")?;
    refused(&mut fixture)?;
    preview::drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn routed_input_during_an_overview_is_refused() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    overview::present_grid(&mut fixture, 3)?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    fixture.commit(4)?;
    fixture.settle_pixels(overview::grid_pixels)?;
    refused(&mut fixture)?;
    fixture.finish()
}

#[test]
fn gestures_for_another_main_lifetime_are_refused() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let mut other = fixture.identity;
    other.navigation = other.navigation.next()?;
    for gesture in [MainCanvasGesture::Wheel(1.0), MainCanvasGesture::Cancel] {
        let result = fixture.canvas.input(MainCanvasInput { identity: other, gesture })?;
        assert_eq!(result, RoutedInput::Refused);
    }
    fixture.finish()
}

/// Pump real execution until the routed drag-out's copied original is handed over.
fn copied(fixture: &mut Fixture) -> Result<MainCanvasDragOut, Box<dyn Error>> {
    loop {
        let revision = fixture.wake.revision();
        fixture.pump()?;
        if let Some(drag) = fixture.canvas.take_drag_out() {
            return Ok(drag);
        }
        fixture.wake.wait_after(revision)?;
    }
}

fn drag_out(fixture: &mut Fixture) -> Result<bool, Box<dyn Error>> {
    Ok(fixture.canvas.input_region().ok_or("Committed final has no input region")?.drag_out)
}

#[test]
fn a_fitted_final_offers_drag_out_and_a_pannable_camera_pans_instead() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    assert!(drag_out(&mut fixture)?, "Fit keeps the output within the viewport");
    fixture.viewport(5, json!({"type":"zoom","factor":4}))?;
    fixture.complete(5)?;
    assert!(!drag_out(&mut fixture)?, "a zoomed output pans");
    assert_eq!(route(&mut fixture, &[MainCanvasGesture::DragOut])?, [RoutedInput::Refused]);
    fixture.viewport(6, json!({"type":"fit"}))?;
    fixture.complete(6)?;
    assert!(drag_out(&mut fixture)?);
    fixture.finish()
}

#[test]
fn a_drag_out_hands_over_the_selected_output_name_and_original_bytes() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let routed = route(
        &mut fixture,
        &[
            MainCanvasGesture::Pointer([50.0, 40.0]),
            button(true),
            MainCanvasGesture::Pointer([60.0, 40.0]),
            MainCanvasGesture::DragOut,
        ],
    )?;
    assert_eq!(routed, [RoutedInput::Applied; 4]);
    let drag = copied(&mut fixture)?;
    assert_eq!(drag.identity, fixture.identity);
    assert_eq!(drag.filename, "wide.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(drag.bytes.as_slice()));
    let info = decoder.read_info()?.info().clone();
    assert_eq!((info.width, info.height), (320, 160), "the exact selected original");
    assert!(fixture.canvas.take_drag_out().is_none(), "handed over at most once");
    drop(drag);
    // The navigation gesture ended, so the camera still accepts commands.
    read(&mut fixture, 5)?;
    fixture.finish()
}

#[test]
fn a_replaced_selection_drops_its_pending_drag_out() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([50.0, 40.0]), MainCanvasGesture::DragOut])?;
    fixture.select(5, "tall.png")?;
    fixture.complete(5)?;
    for _ in 0..8 {
        fixture.pump()?;
        assert!(fixture.canvas.take_drag_out().is_none(), "the replaced output never exports");
    }
    fixture.finish()
}
