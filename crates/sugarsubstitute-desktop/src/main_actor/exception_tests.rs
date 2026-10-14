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
    decode_observation(owner, Observation { kind: Kind::Command, command: Some(command), payload })
}

fn malformed(
    result: Result<Effect, (DesktopError, &'static str)>,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let Err((error, kind)) = result else { return Err("Owned evaluation failure accepted".into()) };
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
    let mut owner = loaded()?;
    let target = owner.poll_probe()?.ok_or("Missing owned evaluation")?;
    malformed(
        completion(
            &mut owner,
            Command::Probe(target),
            r#"{"result":{"type":"string","value":"null"},"exceptionDetails":{"exceptionId":1,"text":"owned sample failed","lineNumber":0,"columnNumber":0,"executionContextId":7}}"#,
        ),
        "Probe",
    )?;
    assert_eq!(owner.poll_probe()?, None);
    assert!(!owner.ready());
    assert!(!owner.dom_result(target, None)?);
    let next = owner.poll_probe()?.ok_or("Issued target custody changed")?;
    assert_eq!(next.operation, target.operation + 1);
    assert_eq!(next.identity, target.identity);
    Ok(())
}

#[test]
fn exact_owned_barrier_exception_cannot_transfer_a_positive_dom_receipt()
-> Result<(), Box<dyn Error>> {
    let mut owner = loaded()?;
    let target = owner.poll_probe()?.ok_or("Missing owned evaluation")?;
    assert!(owner.dom_result(target, Some(positive(target)?))?);
    malformed(
        completion(
            &mut owner,
            Command::Barrier(target),
            r#"{"result":{"type":"number","value":0},"exceptionDetails":{"exceptionId":2,"text":"owned barrier failed","lineNumber":0,"columnNumber":0,"executionContextId":7}}"#,
        ),
        "Barrier",
    )?;
    assert_eq!(owner.poll_probe()?, None);
    assert!(!owner.ready());
    Ok(())
}

#[test]
fn native_process_context_and_navigation_failures_stay_sticky_after_positive_dom()
-> Result<(), Box<dyn Error>> {
    for (kind, payload, expected) in [
        (Kind::ProcessFailed { kind: 1 }, "", ReadinessFault::SetupFailed),
        (Kind::ContextDestroyed, r#"{"executionContextId":7}"#, ReadinessFault::ContextLost),
        (Kind::ContextsCleared, "{}", ReadinessFault::ContextLost),
        (Kind::Navigation { succeeded: false }, "", ReadinessFault::NativeNavigationFailed),
    ] {
        let mut owner = loaded()?;
        let target = owner.poll_probe()?.ok_or("Missing owned evaluation")?;
        assert!(owner.dom_result(target, Some(positive(target)?))?);
        assert!(consume(&mut owner, kind, payload).is_err());
        assert_eq!(owner.poll_probe(), Err(expected));
        assert_eq!(owner.barrier_complete(target), Err(expected));
        assert!(!owner.ready());
    }
    Ok(())
}

#[test]
fn malformed_or_foreign_owned_completions_cannot_replace_current_native_evidence()
-> Result<(), Box<dyn Error>> {
    let mut owner = loaded()?;
    let target = owner.poll_probe()?.ok_or("Missing owned evaluation")?;
    let mut foreign = target;
    foreign.identity.launch = LaunchId::new(2)?;
    let Err((cause, kind)) = completion(
        &mut owner,
        Command::Probe(foreign),
        r#"{"result":{"type":"string","value":"null"}}"#,
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
        assert_eq!(owner.poll_probe()?, None);
        assert!(!owner.ready());
    }
    assert!(!owner.dom_result(target, None)?);
    assert_eq!(
        owner.poll_probe()?.ok_or("Current target custody lost")?.operation,
        target.operation + 1
    );
    Ok(())
}
