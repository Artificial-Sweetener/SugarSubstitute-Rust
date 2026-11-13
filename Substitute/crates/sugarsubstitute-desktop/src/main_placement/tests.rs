// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify normal placement, reference-frame conversion and bounded observation behavior.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type TestResult = Result<(), Box<dyn Error>>;

fn monitor(name: &str, x: f64, dpi: f64) -> Monitor {
    Monitor {
        name: name.into(),
        work: Rect { x, y: 40.0, width: 1920.0, height: 1040.0 },
        dpi,
        primary: x == 0.0,
    }
}

fn normal() -> Rect {
    Rect { x: 120.0, y: 160.0, width: 1280.0, height: 800.0 }
}

fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

#[test]
fn placement_normal_roundtrip_uses_outer_screen_bounds() -> TestResult {
    let monitor = monitor("primary", 0.0, 96.0);
    let saved = snapshot(normal(), false, &monitor)?;
    let plan = restore(&saved, &[monitor])?;
    assert_eq!(plan.bounds, normal());
    assert!(!plan.maximized);
    assert!(!plan.adjusted);
    Ok(())
}

#[test]
fn placement_negative_monitor_origin_survives_roundtrip() -> TestResult {
    let monitor = monitor("left", -1920.0, 144.0);
    let bounds = Rect { x: -1800.0, y: 100.0, width: 1200.0, height: 900.0 };
    let saved = snapshot(bounds, false, &monitor)?;
    assert!(saved.bounds.x < 0.0);
    assert_eq!(restore(&saved, &[monitor])?.bounds, bounds);
    Ok(())
}

#[test]
fn placement_changed_dpi_preserves_logical_size_and_relative_offset() -> TestResult {
    let previous = monitor("left", -1920.0, 96.0);
    let bounds = Rect { x: -1800.0, y: 120.0, width: 600.0, height: 400.0 };
    let saved = snapshot(bounds, false, &previous)?;
    let mut target = monitor("left", -2560.0, 192.0);
    target.work = Rect { x: -2560.0, y: 80.0, width: 2560.0, height: 1400.0 };
    let plan = restore(&saved, &[target])?;
    assert_eq!(plan.bounds, Rect { x: -2320.0, y: 240.0, width: 1200.0, height: 800.0 });
    assert!(plan.adjusted);
    Ok(())
}

#[test]
fn placement_removed_monitor_uses_nearest_current_work_area() -> TestResult {
    let saved =
        snapshot(Rect { x: -1800.0, ..normal() }, true, &monitor("removed", -1920.0, 96.0))?;
    let plan = restore(&saved, &[monitor("far", 4000.0, 96.0), monitor("primary", 0.0, 96.0)])?;
    assert_eq!(plan.bounds, normal());
    assert!(plan.adjusted);
    assert!(plan.maximized);
    Ok(())
}

#[test]
fn placement_monitor_identity_precedes_physical_distance() -> TestResult {
    let saved = snapshot(normal(), false, &monitor("moved", 0.0, 96.0))?;
    let plan = restore(&saved, &[monitor("other", 0.0, 96.0), monitor("moved", 1920.0, 96.0)])?;
    assert_eq!(plan.bounds.x, 2040.0);
    Ok(())
}

#[test]
fn placement_work_area_change_keeps_offset_without_repeated_taskbar_creep() -> TestResult {
    let old = monitor("primary", 0.0, 96.0);
    let mut current = old.clone();
    current.work.x = 48.0;
    current.work.y = 0.0;
    current.work.width -= 48.0;
    let saved = snapshot(normal(), false, &old)?;
    let first = restore(&saved, std::slice::from_ref(&current))?;
    assert_eq!(first.bounds.x, 168.0);
    assert_eq!(first.bounds.y, 120.0);
    let recaptured = snapshot(first.bounds, false, &current)?;
    assert_eq!(restore(&recaptured, &[current])?.bounds, first.bounds);
    Ok(())
}

#[test]
fn placement_oversized_and_offscreen_bounds_fit_work_area() -> TestResult {
    let current = monitor("primary", 0.0, 96.0);
    let saved =
        snapshot(Rect { x: -5000.0, y: 9000.0, width: 8000.0, height: 6000.0 }, false, &current)?;
    let plan = restore(&saved, std::slice::from_ref(&current))?;
    assert_eq!(plan.bounds, current.work);
    assert!(plan.adjusted);
    Ok(())
}

#[test]
fn placement_small_bounds_expand_to_existing_logical_minimum() -> TestResult {
    let current = monitor("primary", 0.0, 192.0);
    let saved = snapshot(Rect { width: 10.0, height: 10.0, ..normal() }, false, &current)?;
    let plan = restore(&saved, &[current])?;
    assert_eq!([plan.bounds.width, plan.bounds.height], [608.0, 480.0]);
    Ok(())
}

#[test]
fn placement_smaller_work_area_wins_over_minimum_without_panicking() -> TestResult {
    let current = Monitor {
        work: Rect { x: 0.0, y: 0.0, width: 200.0, height: 150.0 },
        ..monitor("small", 0.0, 192.0)
    };
    let saved = snapshot(normal(), false, &current)?;
    assert_eq!(restore(&saved, std::slice::from_ref(&current))?.bounds, current.work);
    Ok(())
}

#[test]
fn placement_invalid_or_absent_monitor_data_is_rejected() -> TestResult {
    let current = monitor("primary", 0.0, 96.0);
    let saved = snapshot(normal(), false, &current)?;
    assert!(restore(&saved, &[]).is_err());
    for dpi in [f64::NAN, f64::INFINITY, 0.0, 961.0] {
        let bad = Monitor { dpi, ..current.clone() };
        assert!(restore(&saved, std::slice::from_ref(&bad)).is_err());
        assert!(snapshot(normal(), false, &bad).is_err());
    }
    for name in ["", "nul\0monitor", "line\nmonitor"] {
        assert!(restore(&saved, &[Monitor { name: name.into(), ..current.clone() }]).is_err());
    }
    Ok(())
}

#[test]
fn placement_nonfinite_extreme_and_malformed_bounds_are_rejected() -> TestResult {
    let current = monitor("primary", 0.0, 96.0);
    for bounds in [
        Rect { x: f64::NAN, ..normal() },
        Rect { width: -1.0, ..normal() },
        Rect { x: f64::from(i32::MAX), ..normal() },
    ] {
        assert!(snapshot(bounds, false, &current).is_err());
    }
    let mut saved = snapshot(normal(), false, &current)?;
    saved.version = 2;
    assert!(restore(&saved, &[current]).is_err());
    Ok(())
}

#[test]
fn placement_event_bursts_coalesce_and_hidden_setup_cannot_publish() -> TestResult {
    let mut events = Observations::default();
    events.changed();
    events.changed();
    events.changed();
    assert!(!events.pending(false));
    assert!(events.pending(true));
    let saved = snapshot(normal(), false, &monitor("primary", 0.0, 96.0))?;
    let first = events.capture(identity()?, None, saved.clone())?.ok_or("missing sample")?;
    assert_eq!(first.sequence.get(), 1);
    assert!(!events.pending(true));
    events.changed();
    assert!(events.capture(identity()?, None, saved)?.is_none());
    Ok(())
}

#[test]
fn placement_final_capture_is_correlated_even_when_geometry_is_unchanged() -> TestResult {
    let mut events = Observations::default();
    let saved = snapshot(normal(), true, &monitor("primary", 0.0, 96.0))?;
    assert!(events.capture(identity()?, None, saved.clone())?.is_some());
    let request = NonZeroU64::new(7).ok_or("invalid request")?;
    let last =
        events.capture(identity()?, Some(request), saved.clone())?.ok_or("missing final sample")?;
    assert_eq!(last.sequence.get(), 2);
    assert_eq!(last.capture_id, Some(request));
    assert_eq!(last.identity, identity()?);
    assert_eq!(last.placement, saved);
    assert!(events.capture(identity()?, Some(request), saved).is_err());
    Ok(())
}

#[test]
fn placement_destroyed_peer_and_exhausted_sequence_cannot_publish() -> TestResult {
    let saved = snapshot(normal(), false, &monitor("primary", 0.0, 96.0))?;
    let mut events = Observations::default();
    events.changed();
    events.close();
    events.changed();
    assert!(!events.available());
    assert!(!events.pending(true));
    assert!(events.capture(identity()?, None, saved.clone()).is_err());
    let mut exhausted = Observations { sequence: u64::MAX, ..Default::default() };
    assert!(exhausted.capture(identity()?, None, saved).is_err());
    Ok(())
}

#[test]
fn placement_failed_read_is_observed_once_until_another_real_change() {
    let mut events = Observations::default();
    events.changed();
    events.failed();
    assert!(!events.pending(true));
    assert!(events.available());
    events.changed();
    assert!(events.pending(true));
}
