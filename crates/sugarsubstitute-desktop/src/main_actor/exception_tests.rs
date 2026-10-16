// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact owned evaluation and native lifecycle rejection after removing unowned exceptions.

use super::*;
use crate::main_readiness_types::ReadinessFault;

fn completion(
    owner: &mut MainReadiness,
    command: Command,
    payload: &str,
) -> Result<Effect, (DesktopError, &'static str)> {
    decode_observation(
        owner,
        Observation { kind: Kind::Command, command: Some(command), payload },
        std::time::Instant::now(),
    )
}

fn malformed(
    result: Result<Effect, (DesktopError, &'static str)>,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let Err((error, kind)) = result else {
        return Err("Owned evaluation failure accepted".into());
    };
    assert_eq!(kind, expected);
    assert_eq!(
        error.source().and_then(|cause| cause.downcast_ref::<ReadinessFault>()),
        Some(&ReadinessFault::MalformedObservation)
    );
    Ok(())
}

fn positive(target: ProbeTarget) -> Result<ReadinessReceipt, Box<dyn Error>> {
    Ok(ReadinessReceipt::parse(format!(r#"{{"schema_version":1,"launch_id":{},"navigation_generation":{},"outcome":{{"status":"ready","evidence":{{"frontend_version":"1.47.11","sugarcubes_version":"0.15.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}}}}"#,
        target.identity.launch.get(), target.identity.navigation.get()).as_bytes())?)
}

#[test]
fn exact_owned_probe_exception_rejects_and_keeps_issued_target_without_native_effect()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing owned evaluation")?;
    malformed(
        completion(
            &mut owner,
            Command::Probe(target),
            r#"{"result":{"type":"string","value":"{\"incomplete\":1}"},"exceptionDetails":{"exceptionId":1,"text":"owned sample failed","lineNumber":0,"columnNumber":0,"executionContextId":7}}"#,
        ),
        "Probe",
    )?;
    assert_eq!(owner.poll_probe(now)?, None);
    assert!(!owner.ready());
    assert!(!owner.dom_result(target, None, now)?);
    let next = owner
        .poll_probe(now + std::time::Duration::from_millis(100))?
        .ok_or("Issued target custody changed")?;
    assert_eq!(next.operation, target.operation + 1);
    assert_eq!(next.identity, target.identity);
    Ok(())
}

#[test]
fn exact_owned_barrier_exception_cannot_transfer_a_positive_dom_receipt()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing owned evaluation")?;
    assert!(owner.dom_result(target, Some(positive(target)?), now)?);
    malformed(
        completion(
            &mut owner,
            Command::Barrier(target),
            r#"{"result":{"type":"number","value":0},"exceptionDetails":{"exceptionId":2,"text":"owned barrier failed","lineNumber":0,"columnNumber":0,"executionContextId":7}}"#,
        ),
        "Barrier",
    )?;
    assert_eq!(owner.poll_probe(now)?, None);
    assert!(!owner.ready());
    Ok(())
}

#[test]
fn native_process_context_and_navigation_failures_stay_sticky_after_positive_dom()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    for (kind, payload, expected) in [
        (Kind::ProcessFailed { kind: 1 }, "", ReadinessFault::SetupFailed),
        (Kind::ContextDestroyed, r#"{"executionContextId":7}"#, ReadinessFault::ContextLost),
        (Kind::ContextsCleared, "{}", ReadinessFault::ContextLost),
        (Kind::Navigation { succeeded: false }, "", ReadinessFault::NativeNavigationFailed),
    ] {
        let mut owner = loaded()?;
        let target = owner.poll_probe(now)?.ok_or("Missing owned evaluation")?;
        assert!(owner.dom_result(target, Some(positive(target)?), now)?);
        assert!(consume(&mut owner, kind, payload).is_err());
        assert_eq!(owner.poll_probe(now), Err(expected));
        assert_eq!(owner.barrier_complete(target), Err(expected));
        assert!(!owner.ready());
    }
    Ok(())
}

#[test]
fn malformed_or_foreign_owned_completions_cannot_replace_current_native_evidence()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing owned evaluation")?;
    let mut foreign = target;
    foreign.identity.launch = LaunchId::new(2)?;
    let Err((cause, kind)) = completion(
        &mut owner,
        Command::Probe(foreign),
        r#"{"result":{"type":"string","value":"{\"incomplete\":1}"}}"#,
    ) else {
        return Err("Foreign issued target accepted".into());
    };
    assert_eq!(kind, "Probe");
    assert_eq!(
        cause.source().and_then(|cause| cause.downcast_ref::<ReadinessFault>()),
        Some(&ReadinessFault::StaleProbe)
    );
    for (command, payload, kind) in [
        (Command::Probe(target), r#"{"result":{"type":"boolean","value":true}}"#, "Probe"),
        (Command::Barrier(target), r#"{"result":{"type":"string","value":"0"}}"#, "Barrier"),
        (Command::Barrier(target), r#"{"result":{"type":"number","value":1}}"#, "Barrier"),
    ] {
        malformed(completion(&mut owner, command, payload), kind)?;
        assert_eq!(owner.poll_probe(now)?, None);
        assert!(!owner.ready());
    }
    assert!(!owner.dom_result(target, None, now)?);
    assert_eq!(
        owner
            .poll_probe(now + std::time::Duration::from_millis(100))?
            .ok_or("Current target custody lost")?
            .operation,
        target.operation + 1
    );
    Ok(())
}

#[test]
fn typed_incomplete_result_retains_original_retry_and_cannot_publish_receipt()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing actual evaluation")?;
    let payload = r#"{"result":{"type":"string","value":"{\"incomplete\":6144}"}}"#;
    let effect = decode::consume(
        &mut owner,
        Observation { kind: Kind::Command, command: Some(Command::Probe(target)), payload },
        now,
    )?;
    assert!(matches!(effect, Effect::Incomplete(bits)
        if bits.bits() == 6144));
    assert!(!owner.ready());
    assert_eq!(owner.poll_probe(now)?, None);
    let due = now + std::time::Duration::from_millis(100);
    assert_eq!(owner.next_probe_deadline(), Some(due));
    assert_eq!(
        owner.poll_probe(due)?.ok_or("Missing due evaluation")?.operation,
        target.operation + 1
    );
    Ok(())
}

#[test]
fn incomplete_envelopes_reject_unknown_empty_or_unbounded_conditions_and_receipt_fields()
-> Result<(), Box<dyn Error>> {
    for text in [
        r#"{"incomplete":0}"#,
        r#"{"incomplete":32768}"#,
        r#"{"incomplete":65536}"#,
        r#"{"incomplete":-1}"#,
        r#"{"incomplete":1,"incomplete":2}"#,
        r#"{"incomplete":1,"outcome":{}}"#,
        r#"{"incomplete":"1"}"#,
        "null",
    ] {
        let now = std::time::Instant::now();
        let mut owner = loaded()?;
        let target = owner.poll_probe(now)?.ok_or("Missing actual evaluation")?;
        let payload = serde_json::json!({"result":{"type":"string","value":text}}).to_string();
        assert!(completion(&mut owner, Command::Probe(target), &payload).is_err());
        assert!(!owner.ready());
        assert!(owner.next_probe_deadline().is_none());
        assert!(owner.poll_probe(now)?.is_none());
    }
    Ok(())
}

#[test]
fn actual_receipt_still_requires_the_matching_native_barrier_after_diagnostics()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing actual evaluation")?;
    let receipt = positive(target)?;
    let text = format!(
        r#"{{"schema_version":1,"launch_id":{},"navigation_generation":{},"outcome":{{"status":"ready","evidence":{{"frontend_version":"1.47.11","sugarcubes_version":"0.15.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}}}}"#,
        target.identity.launch.get(),
        target.identity.navigation.get()
    );
    let payload = serde_json::json!({"result":{"type":"string","value":text}}).to_string();
    assert!(matches!(completion(&mut owner, Command::Probe(target), &payload)
        .map_err(|(cause, _)| cause)?, Effect::Barrier(actual) if *actual == target));
    assert!(!owner.ready());
    let barrier = completion(
        &mut owner,
        Command::Barrier(target),
        r#"{"result":{"type":"number","value":0}}"#,
    )
    .map_err(|(cause, _)| cause)?;
    assert!(matches!(barrier, Effect::Receipt(actual) if actual == receipt));
    assert!(owner.ready());
    Ok(())
}
