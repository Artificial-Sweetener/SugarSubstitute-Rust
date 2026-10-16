// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual console projection against bounded protocol envelopes and readiness.

use super::*;

fn metadata() -> serde_json::Value {
    let frames: Vec<_> = (0..48).map(|index| serde_json::json!({
        "functionName":"frontend callback", "scriptId":"9", "url":"http://127.0.0.1:8188/assets/ui.js",
        "lineNumber":index, "columnNumber":0,
    })).collect();
    let mut stack = serde_json::json!({"callFrames": frames});
    for _ in 0..18 {
        stack = serde_json::json!({"description":"async continuation", "callFrames":[], "parent":stack});
    }
    stack
}

fn event(kind: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({"type":kind, "executionContextId":7, "timestamp":12.0,
        "args":arguments, "stackTrace":metadata()})
    .to_string()
}

#[test]
fn valid_console_stack_and_remote_preview_preserve_pending_probe_without_readiness()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    let preview: Vec<_> = (0..48)
        .map(|index| {
            serde_json::json!({
                "name":format!("property{index}"), "type":"string", "value":"ordinary preview",
            })
        })
        .collect();
    let payload = event(
        "log",
        serde_json::json!([{"type":"object", "objectId":"remote.1",
        "preview":{"type":"object", "overflow":false, "properties":preview},
        "value":{"nested":metadata()}}]),
    );
    assert!(payload.len() < crate::main_events::PAYLOAD_BYTES);
    assert!(bounded_json(&payload).is_err());
    let mut owner = loaded()?;
    let target = owner.poll_probe(now)?.ok_or("Missing genuine probe")?;
    assert!(matches!(consume(&mut owner, Kind::Console, &payload)?, Effect::None));
    assert_eq!(owner.poll_probe(now)?, None);
    assert!(!owner.ready());
    assert!(!owner.dom_result(target, None, now)?);
    let next = owner
        .poll_probe(now + std::time::Duration::from_millis(100))?
        .ok_or("Missing next genuine probe")?;
    assert_eq!(next.operation, target.operation + 1);
    assert_eq!(next.identity, target.identity);
    Ok(())
}

#[test]
fn projected_console_keeps_exact_primitive_setup_failures_and_ignores_object_descriptions()
-> Result<(), Box<dyn Error>> {
    let now = std::time::Instant::now();
    for (kind, message, fault) in [
        (
            "error",
            "Error calling extension 'SugarCubes.UI' method 'setup'",
            crate::main_readiness_types::ReadinessFault::SetupFailed,
        ),
        (
            "warning",
            "Extension SugarCubes.UI is disabled.",
            crate::main_readiness_types::ReadinessFault::DisabledExtension,
        ),
    ] {
        let mut owner = loaded()?;
        let payload = event(kind, serde_json::json!([{"type":"string","value":message}]));
        assert!(bounded_json(&payload).is_err());
        let error =
            consume(&mut owner, Kind::Console, &payload).err().ok_or("Missing setup failure")?;
        assert_eq!(
            error.source().and_then(
                |cause| cause.downcast_ref::<crate::main_readiness_types::ReadinessFault>()
            ),
            Some(&fault)
        );
        assert_eq!(owner.poll_probe(now), Err(fault));
        assert!(!owner.ready());
        let mut ordinary = loaded()?;
        consume(
            &mut ordinary,
            Kind::Console,
            &event(
                kind,
                serde_json::json!([
                    {"type":"object","description":message,"value":{"message":message}},
                ]),
            ),
        )?;
        consume(
            &mut ordinary,
            Kind::Console,
            &event(
                "log",
                serde_json::json!([
                    {"type":"string","value":message},
                ]),
            ),
        )?;
        assert!(ordinary.poll_probe(now)?.is_some());
        assert!(!ordinary.ready());
    }
    Ok(())
}

#[test]
fn console_projection_rejects_foreign_duplicate_missing_and_invalid_typed_fields()
-> Result<(), Box<dyn Error>> {
    for payload in [
        r#"{"type":"log","executionContextId":99,"args":[]}"#,
        r#"{"type":"log","executionContextId":0,"args":[]}"#,
        r#"{"type":"log","executionContextId":7,"executionContextId":8,"args":[]}"#,
        r#"{"type":"log","type":"warning","executionContextId":7,"args":[]}"#,
        r#"{"type":"unknown","executionContextId":7,"args":[]}"#,
        r#"{"type":"log","executionContextId":"7","args":[]}"#,
        r#"{"type":"log","executionContextId":7,"args":{}}"#,
        r#"{"type":"log","executionContextId":7}"#,
        r#"{"type":"log","executionContextId":7,"args":[{"type":"unknown"}]}"#,
        r#"{"type":"log","executionContextId":7,"args":[{"type":"string"}]}"#,
        r#"{"type":"log","executionContextId":7,"args":[{"type":"string","value":3}]}"#,
        r#"{"type":"log","executionContextId":7,"args":[{"type":"string","value":"ordinary","value":"duplicate"}]}"#,
    ] {
        let mut owner = loaded()?;
        assert!(consume(&mut owner, Kind::Console, payload).is_err(), "Typed input accepted");
        assert!(!owner.ready());
    }
    Ok(())
}

#[test]
fn console_projection_keeps_argument_and_primitive_string_admission_boundaries()
-> Result<(), Box<dyn Error>> {
    let mut owner = loaded()?;
    let args = vec![serde_json::json!({"type":"undefined"}); 32];
    consume(&mut owner, Kind::Console, &event("log", serde_json::json!(args)))?;
    let too_many = vec![serde_json::json!({"type":"undefined"}); 33];
    assert!(
        consume(&mut owner, Kind::Console, &event("log", serde_json::json!(too_many))).is_err()
    );
    consume(
        &mut owner,
        Kind::Console,
        &event(
            "log",
            serde_json::json!([
                {"type":"string","value":"x".repeat(1024)},
                {"type":"number","value":1}, {"type":"boolean","value":true},
                {"type":"object","value":null}, {"type":"function","objectId":"function.1"},
                {"type":"symbol","description":"Symbol(x)"}, {"type":"bigint","unserializableValue":"1n"},
            ]),
        ),
    )?;
    for value in ["x".repeat(1025), "invalid\0primitive".into()] {
        let mut owner = loaded()?;
        assert!(
            consume(
                &mut owner,
                Kind::Console,
                &event(
                    "log",
                    serde_json::json!([
                        {"type":"string","value":value},
                    ])
                )
            )
            .is_err()
        );
        assert!(!owner.ready());
    }
    Ok(())
}

#[test]
fn console_projection_validates_skipped_json_and_original_byte_envelope()
-> Result<(), Box<dyn Error>> {
    for payload in [
        r#"{"type":"log","executionContextId":7,"args":[],"stackTrace":{"callFrames":[invalid]}}"#
            .to_owned(),
        r#"{"type":"log","executionContextId":7,"args":[],"stackTrace":{"parent":[]}"#.to_owned(),
        r#"{"type":"log","executionContextId":7,"args":[]} trailing"#.to_owned(),
        format!(
            r#"{{"type":"log","executionContextId":7,"args":[],"stackTrace":{{"description":"{}"}}}}"#,
            "x".repeat(crate::main_events::PAYLOAD_BYTES)
        ),
    ] {
        let mut owner = loaded()?;
        assert!(consume(&mut owner, Kind::Console, &payload).is_err());
        assert!(!owner.ready());
    }
    // Other actual event owners retain their original structure rejection.
    assert!(bounded_json(&"[".repeat(17)).is_err());
    assert!(bounded_json(&format!("[{}]", "0,".repeat(513))).is_err());
    Ok(())
}
