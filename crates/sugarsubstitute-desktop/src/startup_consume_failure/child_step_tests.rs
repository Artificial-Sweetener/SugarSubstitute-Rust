// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check that a child step keeps the innermost step and the unchanged original cause.
use super::*;

#[test]
fn every_step_parses_its_own_token_and_refuses_variants() {
    for &step in StartupChildStep::ALL {
        assert_eq!(step.token(), format!("{step:?}"));
        assert_eq!(StartupChildStep::parse(step.token()), Some(step));
    }
    for bad in ["channel", "Channel ", "Channel(1)", "", "PRIVATE"] {
        assert_eq!(StartupChildStep::parse(bad), None);
    }
}

#[test]
fn the_innermost_step_and_original_box_survive_outer_steps() -> Result<(), Box<dyn Error>> {
    let original: Box<dyn Error> = Box::new(std::io::Error::other("PRIVATE"));
    let address = original.as_ref() as *const dyn Error as *const ();
    let inner = StartupChildFailure::retain(StartupChildStep::Control, original);
    let outer = StartupChildFailure::retain(StartupChildStep::Diagnostics, inner);
    assert_eq!(StartupChildFailure::step_of(outer.as_ref()), Some(StartupChildStep::Control));
    let failure = outer.downcast_ref::<StartupChildFailure>().ok_or("Child step lost")?;
    assert_eq!(failure.step(), StartupChildStep::Control);
    assert!(std::ptr::eq(failure.original() as *const dyn Error as *const (), address));
    assert!(!format!("{failure:?} {failure}").contains("PRIVATE"));
    assert_eq!(StartupChildFailure::step_of(&std::io::Error::other("plain")), None);
    Ok(())
}
