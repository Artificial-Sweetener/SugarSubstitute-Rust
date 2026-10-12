// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove actual mapped-view custody survives a post-call cancellation veto.
use super::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};

#[test]
fn symbol_post_map_cancel_retains_actual_view_and_handle_until_checked_retirement() -> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut section = SectionOwner::new();
    section.master.attach(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(&reservation),
    );
    section.master.reserve("fixture_section_create")?;
    // SAFETY: private fixed page-file mapping is immediately adopted by its actual owner.
    let raw = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE,
            0,
            BOOTSTRAP_BYTES as u32,
            None,
        )
    }?;
    adopt(&mut section.master, raw, "fixture_section_create")?;
    let clock = live.rig.clock.clone();
    let core = live.rig.client.core.clone();
    let scope_id = live.rig.scope.id();
    live.rig.clock.on_next_current_thread_observation(Box::new(move || {
        clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(scope_id)));
    }));
    assert!(section.map(FILE_MAP_READ, "fixture_section_map").is_err());
    assert_ne!(section.view, 0);
    assert_ne!(section.master.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 1);
    assert_eq!(live.rig.scope.snapshot().reserved_bytes, crate::symbol_admission::MAIN_BYTES);
    assert!(section.retire(false)?);
    assert_eq!(section.view, 0);
    assert_eq!(section.master.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}
