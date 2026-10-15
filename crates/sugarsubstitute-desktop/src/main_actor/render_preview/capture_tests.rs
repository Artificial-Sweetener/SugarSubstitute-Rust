// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual COM callback obligations with a fake external completion boundary.

use super::*;
use std::{cell::Cell, error::Error};
use sugarsubstitute_contracts::execution::MemoryRelease;
use windows::Win32::Foundation::{E_FAIL, S_OK, STG_E_ACCESSDENIED, STG_E_MEDIUMFULL};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Released;
impl MemoryRelease for Released {
    fn release(&self, _: u64) {}
}
fn callback() -> Result<(Rc<Completion>, ICoreWebView2CapturePreviewCompletedHandler, Rc<Cell<u32>>)>
{
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Released));
    let wakes = Rc::new(Cell::new(0));
    let counted = wakes.clone();
    let events = Events::new(&lease, move || counted.set(counted.get() + 1))?;
    let completion = Rc::new(Completion {
        events,
        stream: Shared::new(&lease)?,
        pending: Cell::new(true),
        invoking: Cell::new(false),
        status: Cell::new(None),
    });
    let handler = Completed { owner: completion.clone() }.into();
    Ok((completion, handler, wakes))
}

#[test]
fn genuine_handler_return_releases_only_its_actual_callback_obligation() -> Result {
    let (owner, handler, wakes) = callback()?;
    assert!(owner.pending.get());
    assert!(!owner.events.in_callback());
    // SAFETY: This calls the real local COM handler with a bounded scalar completion;
    // no WebView/window or physical input is used by the fake native boundary.
    unsafe { handler.Invoke(S_OK)? };
    assert!(!owner.pending.get());
    assert!(!owner.invoking.get());
    assert!(!owner.events.in_callback());
    assert_eq!(owner.status.get(), Some(S_OK));
    assert_eq!(owner.stream.status(), S_OK);
    assert_eq!(wakes.get(), 1);
    Ok(())
}

#[test]
fn native_capture_failure_preserves_hresult_without_fabricating_image() -> Result {
    let (owner, handler, _) = callback()?;
    // SAFETY: Real local handler consumes only this actual simulated HRESULT.
    unsafe { handler.Invoke(E_FAIL)? };
    assert_eq!(owner.status.get(), Some(E_FAIL));
    assert_eq!(owner.stream.status(), E_FAIL);
    assert!(!owner.pending.get());
    assert!(matches!(owner.stream.take(), Err(status) if status == E_FAIL));
    Ok(())
}

#[test]
fn duplicate_native_completion_keeps_first_status_and_invalidates_publication() -> Result {
    let (owner, handler, wakes) = callback()?;
    // SAFETY: Both calls use the same retained real local handler and scalar outputs.
    unsafe {
        handler.Invoke(S_OK)?;
        handler.Invoke(E_FAIL)?;
    }
    assert_eq!(owner.status.get(), Some(S_OK));
    assert_eq!(owner.stream.status(), STG_E_INVALIDFUNCTION);
    assert!(owner.stream.take().is_err());
    assert_eq!(wakes.get(), 2);
    Ok(())
}

#[test]
fn native_success_cannot_replace_a_retained_stream_overflow() -> Result {
    let (owner, handler, _) = callback()?;
    assert_eq!(owner.stream.fail(STG_E_MEDIUMFULL), STG_E_MEDIUMFULL);
    // SAFETY: Fake external native boundary reports success to the real local handler.
    unsafe { handler.Invoke(S_OK)? };
    assert_eq!(owner.status.get(), Some(S_OK));
    assert_eq!(owner.stream.status(), STG_E_MEDIUMFULL);
    assert!(owner.stream.take().is_err());
    Ok(())
}

#[test]
fn completion_freezes_sink_and_late_write_cannot_mutate_completed_bytes() -> Result {
    let (owner, handler, _) = callback()?;
    let sink = stream::interface(&owner.stream);
    let bytes = b"native bytes";
    // SAFETY: Input lives through the exact synchronous COM write; the local callback
    // and stream are the actual owners under test, with no renderer/window effects.
    unsafe {
        sink.Write(bytes.as_ptr().cast(), bytes.len() as u32, None).ok()?;
        handler.Invoke(S_OK)?;
        assert_eq!(sink.Write(bytes.as_ptr().cast(), 1, None), STG_E_ACCESSDENIED);
    }
    assert!(owner.stream.take().is_err());
    Ok(())
}

#[test]
fn closed_event_admission_and_handler_release_do_not_fabricate_callback_completion() -> Result {
    let (owner, handler, wakes) = callback()?;
    owner.events.close_admission();
    let retained_native_handler = handler.clone();
    drop(handler);
    assert!(owner.pending.get());
    assert_eq!(owner.status.get(), None);
    assert_eq!(wakes.get(), 0);
    // SAFETY: The fake renderer retains the actual local handler through its eventual return;
    // closing queue admission never authorizes releasing an outstanding callback obligation.
    unsafe { retained_native_handler.Invoke(S_OK)? };
    assert!(!owner.pending.get());
    assert_eq!(owner.status.get(), Some(S_OK));
    assert_eq!(wakes.get(), 1);
    Ok(())
}

#[test]
fn queued_health_and_reentrant_callback_entry_prevent_preview_sealing_until_actual_drain() -> Result
{
    let (owner, _, _) = callback()?;
    assert!(owner.events.preview_quiescent());
    let callback = owner.events.callback();
    assert!(!owner.events.preview_quiescent());
    drop(callback);
    owner.events.record(crate::main_events::Kind::ContextsCleared, |_| Ok(()));
    assert!(!owner.events.preview_quiescent());
    assert!(owner.events.consume(|event| {
        assert_eq!(event.kind, crate::main_events::Kind::ContextsCleared);
        Ok(())
    })?);
    assert!(owner.events.preview_quiescent());
    owner.events.fail(DesktopError::message("fixture_health", "failed native health"));
    assert!(!owner.events.preview_quiescent());
    assert!(owner.events.take_failure().is_some());
    assert!(!owner.events.preview_quiescent());
    Ok(())
}

#[test]
fn original_counter_boundaries_reject_expiry_foreign_units_and_negative_ticks() -> Result {
    let source = CounterDeadline::new(100, 10)?;
    assert_eq!(admit_counter(source, 10, 99)?, 99);
    for (frequency, ticks) in [(10, 100), (10, 101), (11, 1), (10, -1), (0, 1)] {
        assert!(admit_counter(source, frequency, ticks).is_err());
    }
    assert_eq!(source, CounterDeadline::new(100, 10)?);
    Ok(())
}

#[test]
fn original_four_mib_lease_funds_maximum_request_and_live_preview_claims_together() -> Result {
    use crate::{
        MainCaptionLabels, MainEnvironmentFacts, MainWindowRequest, MainWindowSettings,
        ServedFrontendVersions, main_readiness_storage::DocumentTables,
    };
    use sugarsubstitute_contracts::{
        configuration::{LoopbackOrigin, ProfileId},
        execution::Generation,
        lifecycle::*,
        startup::{LaunchId, MainIdentity},
    };
    let identity = MainIdentity {
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
    };
    let mut path = std::path::PathBuf::new();
    path.try_reserve_exact(MainWindowRequest::PROFILE_PATH_BYTES)?;
    path.push("D:\\owned-profile");
    assert_eq!(path.capacity(), MainWindowRequest::PROFILE_PATH_BYTES);
    let request = MainWindowRequest::new(
        identity,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        ProfileId::new("actual-profile".into())?,
        path,
        ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        MainWindowSettings {
            logical_width: 1280,
            logical_height: 900,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "Minimize",
                maximize: "Maximize",
                restore: "Restore",
                close: "Close",
            },
            recovery: crate::MainRecoveryLabels {
                retry: "Retry",
                retry_ready: "Connection failed. Select here to retry",
            },
        },
        MemoryLease::new(MainWindowRequest::KNOWN_STORAGE_BYTES, Box::new(Released)),
    )?;
    let lease = request.storage_lease();
    let native = lease.claim(512 * 1024)?;
    let receivers = lease.claim(65536)?;
    let environment = lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?;
    let tables = DocumentTables::new(lease)?;
    let events = Events::new(lease, || {})?;
    let preview = Shared::new(lease)?;
    assert_eq!(preview.metadata().bytes(), 8192);
    assert!(Shared::new(lease).is_err());
    drop((native, receivers, environment, tables, events, preview));
    let remaining = MainWindowRequest::KNOWN_STORAGE_BYTES - request.allocated_metadata_bytes();
    let recovered = lease.claim(remaining)?;
    assert_eq!(recovered.bytes(), remaining);
    Ok(())
}
