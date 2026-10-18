// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual bounded observer; faked notifications do not qualify WebView2 ordering.
use super::*;
#[cfg(feature = "k1-native-qualification")]
#[path = "main_readiness_trace_tests.rs"]
mod traces;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
fn identity() -> Result<MainIdentity> {
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
fn owner() -> Result<MainReadiness> {
    Ok(MainReadiness::new(
        identity()?,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        &ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        &MemoryLease::new(4 * 1024 * 1024, Box::new(Release(Arc::default()))),
    )?)
}
fn loaded(owner: &mut MainReadiness) -> Result<ProbeTarget> {
    owner.navigation_started()?;
    owner.frame("root", "loader", None, "http://127.0.0.1:8188/")?;
    owner.context(7, "root", "http://127.0.0.1:8188", true)?;
    owner.navigation_completed(true)?;
    Ok(owner.poll_probe(Instant::now())?.ok_or("Loaded probe target missing")?)
}
fn positive(target: ProbeTarget) -> Result<ReadinessReceipt> {
    Ok(ReadinessReceipt::parse(format!(r#"{{"schema_version":1,"launch_id":{},"navigation_generation":{},"outcome":{{"status":"ready","evidence":{{"frontend_version":"1.47.11","sugarcubes_version":"0.15.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}}}}"#,
        target.identity.launch.get(), target.identity.navigation.get()).as_bytes())?)
}

#[test]
fn positive_dom_waits_for_barrier_and_transfers_exactly_one_receipt() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    assert!(owner.dom_result(target, Some(positive(target)?), Instant::now())?);
    assert!(owner.pending.is_some());
    assert!(!owner.completed);
    assert_eq!(owner.barrier_complete(target)?.navigation(), target.identity.navigation);
    assert!(owner.completed);
    assert!(owner.ready());
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::StaleProbe));
    assert!(owner.poll_probe(Instant::now())?.is_none());
    Ok(())
}

#[test]
fn renderer_snapshot_requires_actual_positive_barrier_and_retains_native_document() -> Result {
    let mut owner = owner()?;
    assert!(owner.render_document().is_none());
    let target = loaded(&mut owner)?;
    assert!(owner.render_document().is_none());
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    assert!(owner.render_document().is_none());
    owner.barrier_complete(target)?;
    let document = owner.render_document().ok_or("Ready document snapshot missing")?;
    assert_eq!(document.identity, target.identity);
    assert_eq!(document.context, target.context);
    assert_eq!(document.frame, target.frame);
    assert_eq!(document.loader, target.loader);
    assert_eq!(document.frame.bytes(), b"root");
    assert_eq!(document.loader.bytes(), b"loader");
    Ok(())
}

#[test]
fn renderer_snapshot_does_not_survive_actual_current_document_health_failure() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    owner.barrier_complete(target)?;
    assert!(owner.render_document().is_some());
    owner.invalidate(ReadinessFault::SetupFailed);
    assert!(owner.render_document().is_none());
    Ok(())
}
#[test]
fn late_caught_setup_error_defeats_positive_dom_and_barrier() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    assert_eq!(
        owner.console(
            7,
            ConsoleKind::Error,
            ConsoleArgument::PrimitiveString(
                "Error calling extension 'SugarCubes.UI' method 'setup'"
            )
        ),
        Err(ReadinessFault::SetupFailed)
    );
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::SetupFailed));
    assert_eq!(owner.poll_probe(Instant::now()), Err(ReadinessFault::SetupFailed));
    assert!(!owner.completed);
    Ok(())
}
#[test]
fn disabled_warning_remains_sticky_after_settings_or_globals_appear_positive() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    assert_eq!(
        owner.console(
            7,
            ConsoleKind::Warning,
            ConsoleArgument::PrimitiveString("Extension SugarCubes.UI is disabled.")
        ),
        Err(ReadinessFault::DisabledExtension)
    );
    assert_eq!(
        owner.dom_result(target, Some(positive(target)?), Instant::now()),
        Err(ReadinessFault::DisabledExtension)
    );
    Ok(())
}
#[test]
fn subframe_and_isolated_console_messages_cannot_supply_top_document_setup_failure() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.frame("child", "child-loader", Some("root"), "http://127.0.0.1:8188/child")?;
    owner.context(8, "child", "http://127.0.0.1:8188", true)?;
    owner.context(9, "root", "", false)?;
    for context in [8, 9] {
        owner.console(
            context,
            ConsoleKind::Error,
            ConsoleArgument::PrimitiveString(
                "Error calling extension 'SugarCubes.UI' method 'setup'",
            ),
        )?;
    }
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    owner.barrier_complete(target)?;
    Ok(())
}
#[test]
fn unknown_console_context_cannot_supply_readiness_diagnostics() -> Result {
    let mut unknown = owner()?;
    loaded(&mut unknown)?;
    assert_eq!(
        unknown.console(99, ConsoleKind::Other, ConsoleArgument::Other),
        Err(ReadinessFault::ContextLost)
    );
    Ok(())
}
#[test]
fn context_loss_cannot_be_repaired_by_positive_sample_or_late_barrier() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    assert_eq!(owner.context_destroyed(7), Err(ReadinessFault::ContextLost));
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::ContextLost));
    Ok(())
}
#[test]
fn foreign_callback_target_leaves_current_pending_evaluation_available() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    let mut foreign = target;
    foreign.identity.navigation = Generation::new(2)?;
    assert_eq!(owner.dom_result(foreign, None, Instant::now()), Err(ReadinessFault::StaleProbe));
    assert!(owner.poll_probe(Instant::now())?.is_none());
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    owner.barrier_complete(target)?;
    Ok(())
}
#[test]
fn matching_barrier_without_actual_dom_result_is_not_readiness() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::MissingDomResult));
    assert!(!owner.completed);
    Ok(())
}
#[test]
fn frame_context_origin_and_loader_changes_invalidate_current_evidence() -> Result {
    let mut origin = owner()?;
    origin.navigation_started()?;
    origin.frame("root", "loader", None, "http://127.0.0.1:8188/")?;
    assert_eq!(
        origin.context(7, "root", "http://localhost:8188", true),
        Err(ReadinessFault::WrongOrigin)
    );
    let mut loader = owner()?;
    loaded(&mut loader)?;
    assert_eq!(
        loader.frame("root", "other-loader", None, "http://127.0.0.1:8188/"),
        Err(ReadinessFault::ContextLost)
    );
    Ok(())
}
#[test]
fn bounded_tables_reject_one_more_context_and_frame_without_growing() -> Result {
    let mut contexts = owner()?;
    contexts.navigation_started()?;
    for id in 1..=64 {
        contexts.context(id, "frame", "", false)?;
    }
    assert_eq!(contexts.context(65, "frame", "", false), Err(ReadinessFault::ObservationOverflow));
    assert_eq!(contexts.tables.contexts.capacity(), 64);
    let mut frames = owner()?;
    frames.navigation_started()?;
    for id in 0..32 {
        frames.frame(&format!("f{id}"), "loader", Some("parent"), "about:blank")?;
    }
    assert_eq!(
        frames.frame("extra", "loader", Some("parent"), "about:blank"),
        Err(ReadinessFault::ObservationOverflow)
    );
    assert_eq!(frames.tables.frames.capacity(), 32);
    Ok(())
}
#[test]
fn admitted_tables_retain_charge_until_owner_drops_and_reject_shared_oversubscription() -> Result {
    let released = Arc::new(AtomicU64::new(0));
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release(released.clone())));
    let blocked = lease.claim(lease.bytes())?;
    assert!(
        MainReadiness::new(
            identity()?,
            LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
            &ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
            &lease
        )
        .is_err()
    );
    drop(blocked);
    let owner = MainReadiness::new(
        identity()?,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        &ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        &lease,
    )?;
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(released.load(Ordering::SeqCst), 4 * 1024 * 1024);
    Ok(())
}
#[test]
fn operation_exhaustion_and_cancellation_do_not_reuse_a_callback_identity() -> Result {
    let now = std::time::Instant::now();
    let mut exhausted = owner()?;
    let first = loaded(&mut exhausted)?;
    exhausted.dom_result(first, None, now)?;
    exhausted.next_operation = u64::MAX;
    assert_eq!(
        exhausted.poll_probe(now + Duration::from_millis(100)),
        Err(ReadinessFault::IdentityExhausted)
    );
    let mut cancelled = owner()?;
    let target = loaded(&mut cancelled)?;
    cancelled.invalidate(ReadinessFault::Cancelled);
    assert_eq!(
        cancelled.dom_result(target, Some(positive(target)?), now),
        Err(ReadinessFault::Cancelled)
    );
    Ok(())
}

#[test]
fn initial_blank_context_clearing_is_not_loss_of_current_document_evidence() -> Result {
    let mut owner = owner()?;
    owner.context(3, "blank", "", true)?;
    owner.navigation_started()?;
    owner.context_destroyed(3)?;
    owner.contexts_cleared()?;
    owner.frame("root", "loader", None, "http://127.0.0.1:8188/")?;
    owner.context(7, "root", "http://127.0.0.1:8188", true)?;
    owner.navigation_completed(true)?;
    let target = owner.poll_probe(Instant::now())?.ok_or("Current target missing")?;
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    assert_eq!(owner.contexts_cleared(), Err(ReadinessFault::ContextLost));
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::ContextLost));
    Ok(())
}

#[test]
fn validated_failure_detail_transfers_without_becoming_positive_readiness() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    let receipt = ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"failed","failure":{"code":"frontend_unavailable","detail":"Original bounded probe failure"}}}"#)?;
    owner.dom_result(target, Some(receipt), Instant::now())?;
    let observed = owner.barrier_complete(target)?;
    assert!(!owner.ready());
    let ReadinessOutcome::Failed { failure } = observed.into_outcome() else {
        return Err("Failure became readiness".into());
    };
    assert_eq!(failure.detail, "Original bounded probe failure");
    Ok(())
}

#[test]
fn child_detachment_preserves_attribution_until_context_retirement_and_root_detachment_fails()
-> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.frame("child", "child-loader", Some("root"), "http://127.0.0.1:8188/child")?;
    owner.context(8, "child", "http://127.0.0.1:8188", true)?;
    owner.frame_detached("child")?;
    owner.context_destroyed(8)?;
    owner.dom_result(target, Some(positive(target)?), Instant::now())?;
    assert_eq!(owner.frame_detached("root"), Err(ReadinessFault::ContextLost));
    assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::ContextLost));
    Ok(())
}

#[path = "main_readiness_probe_tests.rs"]
mod probe;

#[path = "main_readiness_frame_tests.rs"]
mod frame;
