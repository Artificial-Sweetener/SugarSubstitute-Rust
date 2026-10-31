// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise retirement after failed native visual-tree effects.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use tao::{
    event_loop::EventLoopBuilder, platform::windows::EventLoopBuilderExtWindows,
    window::WindowBuilder,
};
use windows::Win32::Foundation::E_FAIL;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn retirement_preserves_failed_mutations_and_commits_after_both_detachments() {
    let mut ledger =
        Ledger { browser_bound: true, tree_bound: true, dirty: false, committed: false };
    let mut actions = Vec::new();
    for expected in [Retire::DetachBrowser, Retire::DetachTree, Retire::Commit] {
        let failed = ledger.step(|action| {
            actions.push(action);
            Err::<(), _>("injected failure")
        });
        assert_eq!(failed, Err("injected failure"));
        assert_eq!(ledger.next(), expected);
        assert_eq!(
            ledger.step(|action| {
                actions.push(action);
                Ok::<_, &str>(())
            }),
            Ok(false)
        );
    }
    assert_eq!(ledger.step(|_| Err::<(), _>("must not run")), Ok(true));
    assert_eq!(
        actions,
        [
            Retire::DetachBrowser,
            Retire::DetachBrowser,
            Retire::DetachTree,
            Retire::DetachTree,
            Retire::Commit,
            Retire::Commit
        ]
    );
}

struct Released(Arc<AtomicU64>);

impl MemoryRelease for Released {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

#[test]
fn injected_device_failure_retires_the_partial_surface_without_native_effects() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::injected_device_failure_retires_the_partial_surface_without_native_effects"
        ),
        qualify_device_failure,
    )
}

/// A failed device leaves no tree to detach, so retirement releases every retained owner at once.
fn qualify_device_failure() -> TestResult {
    let _apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<()>::with_user_event();
    let events = builder.with_any_thread(true).build();
    let window = Rc::new(WindowBuilder::new().with_visible(false).build(&events)?);
    let released = Arc::new(AtomicU64::new(0));
    let lease = MemoryLease::new(65536, Box::new(Released(released.clone())));
    let claim = lease.claim(65536)?;
    let mut composition = Composition::new(window.clone(), claim, DeviceSource::Failing(E_FAIL));
    let failure = composition.advance().err().ok_or("injected device failure was ignored")?;
    assert!(failure.to_string().contains("create_composition_device"), "{failure}");
    assert!(!composition.surface_ready());
    assert!(composition.device.is_none() && composition.target.is_none());
    assert!(composition.retire_step()?, "a failed device has no tree to detach");
    assert!(composition.window.is_none() && composition.claim.is_none());
    drop(composition);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 65536);
    assert_eq!(Rc::strong_count(&window), 1, "the parent window is returned to its owner");
    Ok(())
}
