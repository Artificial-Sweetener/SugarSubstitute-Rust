// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual retained returned values at every partial native construction boundary.
use super::*;
use std::{cell::RefCell, error::Error, rc::Rc};
struct Slot {
    name: &'static str,
    dropped: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.dropped.borrow_mut().push(self.name);
    }
}
struct Port {
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
    dropped: Rc<RefCell<Vec<&'static str>>>,
}
impl Port {
    fn effect(&mut self, name: &'static str) -> Result<(), DesktopError> {
        self.calls.push(name);
        if self.fail == Some(name) {
            return Err(DesktopError::new("surface_boundary", std::io::Error::other(name)));
        }
        Ok(())
    }
    fn slot(&self, name: &'static str) -> Slot {
        Slot { name, dropped: self.dropped.clone() }
    }
}
impl SurfacePort for Port {
    type Window = Slot;
    type Presenter = Slot;
    type Controls = Slot;
    fn window(&mut self) -> Result<Slot, DesktopError> {
        self.effect("window")?;
        Ok(self.slot("window"))
    }
    fn window_adopted(&mut self) -> Result<(), DesktopError> {
        self.effect("window_timing")
    }
    fn presenter(&mut self, window: &Slot) -> Result<Slot, DesktopError> {
        assert_eq!(window.name, "window");
        self.effect("presenter")?;
        Ok(self.slot("presenter"))
    }
    fn presenter_adopted(&mut self) -> Result<(), DesktopError> {
        self.effect("presenter_timing")
    }
    fn controls(&mut self, window: &Slot) -> Result<Slot, DesktopError> {
        assert_eq!(window.name, "window");
        self.effect("controls_inert")?;
        Ok(self.slot("controls"))
    }
    fn initialize_controls(&mut self, controls: &mut Slot) -> Result<(), DesktopError> {
        assert_eq!(controls.name, "controls");
        self.effect("controls_native")
    }
    fn controls_adopted(&mut self) -> Result<(), DesktopError> {
        self.effect("controls_timing")
    }
}

#[test]
fn each_failed_creation_or_observation_preserves_every_already_adopted_value()
-> Result<(), Box<dyn Error>> {
    for (failure, expected) in [
        ("window", [false, false, false]),
        ("window_timing", [true, false, false]),
        ("presenter", [true, false, false]),
        ("presenter_timing", [true, true, false]),
        ("controls_inert", [true, true, false]),
        ("controls_native", [true, true, true]),
        ("controls_timing", [true, true, true]),
    ] {
        let mut fence = Initialization::new();
        let (mut window, mut presenter, mut controls) = (None, None, None);
        let mut port = Port {
            fail: Some(failure),
            calls: Vec::new(),
            dropped: Rc::new(RefCell::new(Vec::new())),
        };
        let cause = fence
            .initialize(&mut window, &mut presenter, &mut controls, &mut port)
            .err()
            .ok_or("Boundary did not fail")?;
        assert_eq!(cause.source().ok_or("Original cause lost")?.to_string(), failure);
        assert_eq!([window.is_some(), presenter.is_some(), controls.is_some()], expected);
        assert!(port.dropped.borrow().is_empty());
        let calls = port.calls.len();
        port.fail = None;
        assert!(fence.initialize(&mut window, &mut presenter, &mut controls, &mut port).is_err());
        assert_eq!(port.calls.len(), calls);
        // Native retirement belongs to the retained outer surface, not this initialization call.
        drop(controls.take());
        drop(presenter.take());
        drop(window.take());
        let expected_drops: Vec<_> = ["controls", "presenter", "window"]
            .into_iter()
            .zip(expected.into_iter().rev())
            .filter_map(|(name, present)| present.then_some(name))
            .collect();
        assert_eq!(*port.dropped.borrow(), expected_drops);
    }
    Ok(())
}

#[test]
fn successful_initialization_has_one_attempt_and_does_not_transfer_retained_slots()
-> Result<(), DesktopError> {
    let mut fence = Initialization::new();
    let (mut window, mut presenter, mut controls) = (None, None, None);
    let mut port =
        Port { fail: None, calls: Vec::new(), dropped: Rc::new(RefCell::new(Vec::new())) };
    fence.initialize(&mut window, &mut presenter, &mut controls, &mut port)?;
    assert!(window.is_some() && presenter.is_some() && controls.is_some());
    assert!(port.dropped.borrow().is_empty());
    let calls = port.calls.len();
    assert!(fence.initialize(&mut window, &mut presenter, &mut controls, &mut port).is_err());
    assert_eq!(port.calls.len(), calls);
    drop(controls);
    drop(presenter);
    drop(window);
    assert_eq!(*port.dropped.borrow(), ["controls", "presenter", "window"]);
    Ok(())
}
