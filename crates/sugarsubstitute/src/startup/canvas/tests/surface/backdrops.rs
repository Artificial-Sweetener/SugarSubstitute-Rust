// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A geometry reply proves that the native owner accepted the current material generation.
use super::*;
use viewport::fixture::Fixture;

/// Hold viewport and routing fixed while changing only disposable presentation material.
fn geometry(sigma: Option<f64>) -> Value {
    let regions = sigma.map_or_else(Vec::new, |sigma| {
        vec![json!({
            "x":8,"y":5,"width":50,"height":24,"cornerRadius":6,"sigma":sigma
        })]
    });
    json!({"geometry": {
        "coordinateSpace":"client","units":"css-pixels","origin":{"x":10,"y":20},
        "viewport":{"width":80,"height":40},"clip":{"x":10,"y":20,"width":80,"height":40},
        "physicalPixelsPerCssPixel":1,"visible":true,"focused":false,"backdrops":regions
    }})
}

#[test]
fn geometry_material_changes_wait_for_matching_native_acceptance() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    for (id, sigma) in [(5, Some(3.0)), (7, Some(5.0)), (9, None)] {
        let previous =
            prepared_frame(&fixture.canvas).ok_or("Shown frame missing")?.content.target();
        let before = fixture
            .canvas
            .session
            .as_ref()
            .ok_or("Session missing")?
            .presented_backdrop_generation()
            .ok_or("Native receipt missing")?;
        fixture.send(id, "surface.geometry", geometry(sigma))?;
        assert!(fixture.canvas.take_command().is_none());
        assert!(fixture.canvas.presented(previous).is_err());
        assert!(
            fixture.canvas.take_command().is_none(),
            "Previous frame acknowledged new material"
        );
        fixture.complete(id)?;
        let current = fixture
            .canvas
            .session
            .as_ref()
            .ok_or("Session missing")?
            .presented_backdrop_generation()
            .ok_or("Current native receipt missing")?;
        assert!(current > before);
        fixture.send(id + 1, "surface.geometry", geometry(sigma))?;
        fixture.take(id + 1, "applied")?;
        assert_eq!(
            fixture
                .canvas
                .session
                .as_ref()
                .ok_or("Session missing")?
                .presented_backdrop_generation(),
            Some(current)
        );
    }
    fixture.finish()?;
    Ok(())
}

#[test]
fn configured_uncommitted_material_does_not_claim_a_native_frame() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.send(4, "surface.geometry", geometry(Some(4.0)))?;
    fixture.take(4, "applied")?;
    assert_eq!(
        fixture.canvas.session.as_ref().ok_or("Session missing")?.presented_backdrop_generation(),
        None
    );
    fixture.send(5, "surface.commit", json!({}))?;
    fixture.complete(5)?;
    assert!(
        fixture
            .canvas
            .session
            .as_ref()
            .ok_or("Session missing")?
            .presented_backdrop_generation()
            .is_some()
    );
    fixture.finish()?;
    Ok(())
}
