// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject malformed and premature web readiness at the protocol owner.

use super::*;
use serde_json::{Value, json};

/// Supply every independently required observation from the installed owners.
fn ready() -> Value {
    json!({
        "schema_version": 1,
        "launch_id": 7,
        "navigation_generation": 3,
        "outcome": {
            "status": "ready",
            "evidence": {
                "frontend_version": "1.47.11",
                "sugarcubes_version": "0.15.0",
                "app_graph_ready": true,
                "canvas_connected": true,
                "loading_finished": true,
                "sugarcubes_registered": true,
                "sugarcubes_ready": true
            }
        }
    })
}

/// Parse the same bytes that a desktop transport will hand to this boundary.
fn parse(value: &Value) -> Result<ReadinessReceipt, ReadinessError> {
    ReadinessReceipt::parse(value.to_string().as_bytes())
}

/// Preserve immutable launch/navigation identity and all successful evidence.
#[test]
fn accepts_complete_installed_frontend_evidence() -> Result<(), Box<dyn Error>> {
    let receipt = parse(&ready())?;
    assert_eq!(receipt.launch(), LaunchId::new(7)?);
    assert_eq!(receipt.navigation(), Generation::new(3)?);
    let ReadinessOutcome::Ready { evidence } = receipt.outcome() else {
        return Err("Expected ready evidence".into());
    };
    assert_eq!(evidence.frontend_version, "1.47.11");
    assert_eq!(evidence.sugarcubes_version, "0.15.0");
    Ok(())
}

/// Registration or an early application event alone cannot establish readiness.
#[test]
fn rejects_each_missing_frontend_or_extension_observation() {
    for field in [
        "app_graph_ready",
        "canvas_connected",
        "loading_finished",
        "sugarcubes_registered",
        "sugarcubes_ready",
    ] {
        let mut value = ready();
        value["outcome"]["evidence"][field] = false.into();
        assert!(matches!(parse(&value), Err(ReadinessError::IncompleteEvidence)), "{field}");
    }
}

/// Fail closed on unknown versions, identities and fields at every envelope level.
#[test]
fn rejects_unknown_schema_fields_and_invalid_identity() {
    let mut value = ready();
    value["schema_version"] = 2.into();
    assert!(matches!(parse(&value), Err(ReadinessError::UnsupportedVersion(2))));
    for field in ["launch_id", "navigation_generation"] {
        let mut value = ready();
        value[field] = 0.into();
        assert!(matches!(parse(&value), Err(ReadinessError::Identity(IdentityError::Zero))));
        value[field] = true.into();
        assert!(matches!(parse(&value), Err(ReadinessError::Json(_))));
    }
    for path in [vec![], vec!["outcome"], vec!["outcome", "evidence"]] {
        let mut value = ready();
        let mut object = &mut value;
        for field in path {
            object = &mut object[field];
        }
        object["native_command"] = "delete".into();
        assert!(matches!(parse(&value), Err(ReadinessError::Json(_))));
    }
}

/// Ambiguous duplicate fields and numeric coercion cannot select an accepted identity.
#[test]
fn rejects_duplicate_fields_numeric_coercion_and_missing_evidence() -> Result<(), Box<dyn Error>> {
    let source = ready().to_string();
    for (original, replacement) in [
        ("\"launch_id\":7", "\"launch_id\":7,\"launch_id\":8"),
        ("\"status\":\"ready\"", "\"status\":\"ready\",\"status\":\"failed\""),
        ("\"canvas_connected\":true", "\"canvas_connected\":true,\"canvas_connected\":false"),
        ("\"launch_id\":7", "\"launch_id\":7.0"),
    ] {
        let ambiguous = source.replacen(original, replacement, 1);
        assert_ne!(ambiguous, source);
        assert!(matches!(
            ReadinessReceipt::parse(ambiguous.as_bytes()),
            Err(ReadinessError::Json(_))
        ));
    }
    let mut value = ready();
    let fields = value["outcome"]["evidence"].as_object_mut().ok_or("Expected evidence object")?;
    assert!(fields.remove("canvas_connected").is_some());
    assert!(matches!(parse(&value), Err(ReadinessError::Json(_))));
    value["outcome"] = json!({"status": "failed", "failure": {
        "code": "timed_out", "detail": "deadline", "native_command": "delete"
    }});
    assert!(matches!(parse(&value), Err(ReadinessError::Json(_))));
    Ok(())
}

/// Check the byte limit before the JSON decoder and preserve decoder causes.
#[test]
fn bounds_wire_bytes_and_preserves_malformed_json_cause() -> Result<(), Box<dyn Error>> {
    let source = ready().to_string();
    let mut exact = source.into_bytes();
    exact.resize(ReadinessReceipt::MAX_BYTES, b' ');
    ReadinessReceipt::parse(&exact)?;
    exact.push(b' ');
    assert!(matches!(ReadinessReceipt::parse(&exact), Err(ReadinessError::TooLarge)));
    let error = ReadinessReceipt::parse(b"{").err().ok_or("Expected invalid JSON")?;
    assert!(error.source().is_some());
    let mut value = ready();
    value["untrusted_secret_field"] = true.into();
    let error = parse(&value).err().ok_or("Expected unknown-field rejection")?;
    assert!(!error.to_string().contains("untrusted_secret_field"));
    assert!(error.source().is_some());
    Ok(())
}

/// Restrict observed versions independently of the overall receipt size.
#[test]
fn bounds_observed_version_labels() {
    for field in ["frontend_version", "sugarcubes_version"] {
        for invalid in ["".to_owned(), "v".repeat(65), "1.0\n".into(), "版本".into()] {
            let mut value = ready();
            value["outcome"]["evidence"][field] = invalid.into();
            assert!(matches!(parse(&value), Err(ReadinessError::InvalidVersion)));
        }
        let mut value = ready();
        value["outcome"]["evidence"][field] = "v".repeat(64).into();
        assert!(parse(&value).is_ok());
    }
}

/// Preserve terminal failure without converting its detail into trusted product copy.
#[test]
fn retains_typed_failure_and_bounds_utf8_detail() -> Result<(), Box<dyn Error>> {
    let mut value = ready();
    value["outcome"] = json!({
        "status": "failed",
        "failure": {"code": "sugarcubes_failed", "detail": "设置失败"}
    });
    let receipt = parse(&value)?;
    assert!(matches!(receipt.outcome(), ReadinessOutcome::Failed { failure }
        if failure.code == ReadinessFailure::SugarcubesFailed && failure.detail == "设置失败"));
    value["outcome"]["failure"]["detail"] = "x".repeat(1024).into();
    assert!(parse(&value).is_ok());
    for invalid in ["x".repeat(1025), "bad\0detail".into()] {
        value["outcome"]["failure"]["detail"] = invalid.into();
        assert!(matches!(parse(&value), Err(ReadinessError::InvalidFailureDetail)));
    }
    value["outcome"]["failure"]["code"] = "execute_native".into();
    assert!(matches!(parse(&value), Err(ReadinessError::Json(_))));
    Ok(())
}
