// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise deferred native removal while reentrant host callbacks remain in flight.

use super::*;
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};

/// Keep the fixture lease alive for as long as the events owner retains its claim.
pub(in crate::main_actor::composition) struct FixtureEvents {
    pub(in crate::main_actor::composition) events: Rc<Events>,
    _lease: MemoryLease,
}

/// Build an isolated event owner whose wake count is observable by the test.
pub(in crate::main_actor::composition) fn fixture_events(
    wakes: Rc<Cell<usize>>,
) -> Result<FixtureEvents, Box<dyn std::error::Error>> {
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let events = Events::new(&lease, move || wakes.set(wakes.get() + 1))?;
    Ok(FixtureEvents { events, _lease: lease })
}

#[test]
fn retirement_waits_until_native_callbacks_return() -> Result<(), Box<dyn std::error::Error>> {
    let wakes = Rc::new(Cell::new(0));
    let fixture = fixture_events(wakes.clone())?;
    let fence = CallbackFence::default();
    let outer = fence.enter(&fixture.events)?;
    let inner = fence.enter(&fixture.events)?;
    let removed = Cell::new(false);
    assert!(!retire_when_quiet(&fence, || {
        removed.set(true);
        Ok(())
    })?);
    assert!(!removed.get());
    drop(inner);
    assert_eq!(wakes.get(), 0);
    drop(outer);
    assert_eq!(wakes.get(), 1);
    assert!(retire_when_quiet(&fence, || {
        removed.set(true);
        Ok(())
    })?);
    assert!(removed.get());
    Ok(())
}

#[test]
fn failed_removal_remains_retryable_without_a_spurious_wake()
-> Result<(), Box<dyn std::error::Error>> {
    let wakes = Rc::new(Cell::new(0));
    let fixture = fixture_events(wakes.clone())?;
    let fence = CallbackFence::default();
    assert!(
        retire_when_quiet(&fence, || Err(DesktopError::message("fixture", "injected"))).is_err()
    );
    drop(fence.enter(&fixture.events)?);
    assert_eq!(wakes.get(), 0);
    assert!(retire_when_quiet(&fence, || Ok(()))?);
    Ok(())
}
