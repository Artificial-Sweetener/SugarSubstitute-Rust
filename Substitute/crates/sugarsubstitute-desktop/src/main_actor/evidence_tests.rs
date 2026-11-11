// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize actual decoded frame/context/receipt failures with deterministic owner input.

use super::*;
#[path = "console_tests.rs"]
mod console;
#[path = "exception_tests.rs"]
mod exceptions;
use crate::{MainIdentity, ServedFrontendVersions};
use std::error::Error;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn owner() -> Result<MainReadiness, Box<dyn Error>> {
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
    Ok(MainReadiness::new(
        identity,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        &ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        &MemoryLease::new(4 * 1024 * 1024, Box::new(Release)),
    )?)
}
fn consume(owner: &mut MainReadiness, kind: Kind, payload: &str) -> Result<Effect, DesktopError> {
    decode::consume(owner, Observation { kind, command: None, payload }, std::time::Instant::now())
}

/// Admit actual decoded root/context/navigation facts for all evidence owner regressions.
fn loaded() -> Result<MainReadiness, Box<dyn Error>> {
    let mut owner = owner()?;
    owner.navigation_started()?;
    consume(
        &mut owner,
        Kind::Frame,
        r#"{"frame":{"id":"root","loaderId":"loader","url":"http://127.0.0.1:8188/"}}"#,
    )?;
    consume(
        &mut owner,
        Kind::Context,
        r#"{"context":{"id":7,"origin":"http://127.0.0.1:8188","auxData":{"frameId":"root","isDefault":true}}}"#,
    )?;
    consume(&mut owner, Kind::Navigation { succeeded: true }, "")?;
    Ok(owner)
}
#[test]
fn native_json_bounds_reject_deep_and_large_structure_before_decoding() {
    assert!(bounded_json(&"[".repeat(17)).is_err());
    assert!(bounded_json(&format!("[{}]", "0,".repeat(513))).is_err());
    assert!(
        bounded_json(&format!("\"{}\"", "x".repeat(crate::main_events::PAYLOAD_BYTES))).is_err()
    );
    assert!(bounded_json(r#"{"message":"escaped \" [ brackets in text"}"#).is_ok());
}
#[test]
fn duplicate_native_context_identity_is_rejected_by_typed_decoder() -> Result<(), Box<dyn Error>> {
    let mut owner = owner()?;
    owner.navigation_started()?;
    assert!(consume(&mut owner, Kind::Context,
        r#"{"context":{"id":7,"id":8,"origin":"http://127.0.0.1:8188","auxData":{"frameId":"root","isDefault":true}}}"#).is_err());
    Ok(())
}
#[test]
fn actual_decoded_setup_failure_remains_sticky_through_later_positive_inputs()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = owner()?;
    owner.navigation_started()?;
    consume(
        &mut owner,
        Kind::Frame,
        r#"{"frame":{"id":"root","loaderId":"loader","url":"http://127.0.0.1:8188/"}}"#,
    )?;
    consume(
        &mut owner,
        Kind::Context,
        r#"{"context":{"id":7,"origin":"http://127.0.0.1:8188","auxData":{"frameId":"root","isDefault":true}}}"#,
    )?;
    consume(&mut owner, Kind::Navigation { succeeded: true }, "")?;
    assert!(consume(&mut owner, Kind::Console,
        r#"{"type":"error","executionContextId":7,"args":[{"type":"string","value":"Error calling extension 'SugarCubes.UI' method 'setup'"}]}"#).is_err());
    assert!(owner.poll_probe(now).is_err());
    assert!(!owner.ready());
    Ok(())
}
#[test]
fn empty_and_foreign_completion_metadata_cannot_publish_receipt() -> Result<(), Box<dyn Error>> {
    let mut owner = owner()?;
    assert!(consume(&mut owner, Kind::Command, "{}").is_err());
    assert!(!owner.ready());
    Ok(())
}

#[test]
fn malformed_frame_keeps_actual_observation_kind_and_original_decode_cause()
-> Result<(), Box<dyn Error>> {
    let mut owner = owner()?;
    owner.navigation_started()?;
    let Err((cause, kind)) = decode_observation(
        &mut owner,
        Observation {
            kind: Kind::Frame,
            command: None,
            payload: r#"{"frame":{"id":"root","url":"http://127.0.0.1:8188/"}}"#,
        },
        std::time::Instant::now(),
    ) else {
        return Err("Missing actual native frame decoder failure".into());
    };
    assert_eq!(kind, "Frame");
    let json = cause
        .source()
        .and_then(|source| source.downcast_ref::<serde_json::Error>())
        .ok_or("Original native JSON cause lost")?;
    assert_eq!(json.classify(), serde_json::error::Category::Data);
    assert!(!owner.ready());
    Ok(())
}
