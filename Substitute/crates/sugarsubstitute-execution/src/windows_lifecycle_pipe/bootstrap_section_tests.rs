// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove actual mapped-view custody survives a post-call cancellation veto.
use super::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};

#[test]
fn retirement_waits_for_capture_borrow_without_reporting_a_native_failure() -> TestResult {
    let rig = crate::runtime_test_support::Rig::new()?;
    let (_scope, context) = crate::runtime_test_support::admitted_context(&rig, 2, 1)?;
    let mut section = SectionOwner::new();
    // SAFETY: this private mapping is immediately adopted by its physical owner.
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
    section.map(FILE_MAP_READ, "fixture_section_map")?;
    let original_view = section.view;
    let original_handle = section.master.value();
    let mut mapping = crate::windows_lifecycle_pipe::startup_mapping::StartupMapping::new(
        context.memory().claim(4096)?,
        Signal::new(&rig.client.core),
    );
    // SAFETY: Section owns the actual immutable view until the last borrow returns.
    unsafe { mapping.adopt(section.view, BOOTSTRAP_BYTES) }?;
    let capture = mapping.loan(0..16)?.into_retention();
    section.startup = Some(mapping);
    let waiting = section.retire(false);
    assert_eq!(section.view, original_view);
    assert_eq!(section.master.value(), original_handle);
    // Return the real borrow before assertions can unwind the native map owner.
    drop(capture);
    assert!(section.retire(false)?);
    assert!(matches!(waiting, Ok(false)), "{waiting:?}");
    assert_eq!(section.view, 0);
    assert_eq!(section.master.value(), 0);
    assert!(section.retire(false)?);
    Ok(())
}

#[test]
fn symbol_post_map_cancel_retains_actual_view_and_handle_until_checked_retirement() -> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut section = SectionOwner::new();
    section.master.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(&reservation),
    ));
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
