// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn state() -> Result<UpdatesState, UpdatesError> {
    Ok(UpdatesState {
        current_version: UpdatesVersion::new("0.4.0")?,
        channel: UpdatesChannel::Beta,
        last_successful_check_ms: Some(1_700_000_000_000),
        status: UpdatesStatus::Ready(UpdatesVersion::new("0.5.0-beta.1")?),
        report: Some(UpdatesReport::new(
            UpdatesVersion::new("0.5.0-beta.1")?,
            UpdatesPhase::Download,
            vec!["The download was interrupted".into()],
        )?),
        previous_version_running: false,
        workflow_running: true,
        scheduled_restart: UpdatesScheduledRestart::AfterWorkflow,
    })
}

fn encoded(message: &UpdatesHostMessage) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&message.encode(&session()?)?)?)
}

#[test]
fn a_state_encodes_every_schema_field() -> TestResult {
    let message = UpdatesHostMessage::State { revision: 3, update: Box::new(state()?) };
    assert_eq!(
        encoded(&message)?,
        json!({
            "protocol": "sugarcubes.updates", "version": 1, "session": SESSION, "type": "state",
            "revision": 3,
            "update": {
                "currentVersion": "0.4.0", "channel": "beta",
                "lastSuccessfulCheck": {"completedAt": 1_700_000_000_000_u64},
                "status": {"state": "ready", "version": "0.5.0-beta.1"},
                "report": {"version": "0.5.0-beta.1", "phase": "download",
                    "causes": [{"message": "The download was interrupted"}]},
                "previousVersionRunning": false, "workflowRunning": true,
                "scheduledRestart": "after-workflow",
            },
        })
    );
    Ok(())
}

#[test]
fn optional_state_fields_are_omitted_and_every_status_encodes() -> TestResult {
    for (status, expected) in [
        (UpdatesStatus::UpToDate, json!({"state": "up-to-date"})),
        (UpdatesStatus::Checking, json!({"state": "checking"})),
        (
            UpdatesStatus::Available(UpdatesVersion::new("1.0.0")?),
            json!({"state": "available", "version": "1.0.0"}),
        ),
        (
            UpdatesStatus::Failed(UpdatesFailure::Check),
            json!({"state": "failed", "during": "check"}),
        ),
        (
            UpdatesStatus::Failed(UpdatesFailure::Update),
            json!({"state": "failed", "during": "update"}),
        ),
    ] {
        let update = UpdatesState {
            last_successful_check_ms: None,
            report: None,
            status,
            scheduled_restart: UpdatesScheduledRestart::None,
            ..state()?
        };
        let value = encoded(&UpdatesHostMessage::State { revision: 1, update: Box::new(update) })?;
        assert_eq!(value["update"]["status"], expected);
        assert_eq!(value["update"]["scheduledRestart"], "none");
        let fields = value["update"].as_object().ok_or("State is not an object")?;
        assert!(!fields.contains_key("lastSuccessfulCheck") && !fields.contains_key("report"));
    }
    Ok(())
}

#[test]
fn answers_name_their_request() -> TestResult {
    let request = UpdatesRequestName::new("check-1")?;
    assert_eq!(
        encoded(&UpdatesHostMessage::Accepted(request.clone()))?,
        json!({"protocol": "sugarcubes.updates", "version": 1, "session": SESSION,
            "type": "accepted", "request": "check-1"})
    );
    for (reason, token) in [
        (UpdatesRejection::Busy, "busy"),
        (UpdatesRejection::NotReady, "not-ready"),
        (UpdatesRejection::Unavailable, "unavailable"),
    ] {
        let value = encoded(&UpdatesHostMessage::Rejected { request: request.clone(), reason })?;
        assert_eq!(value["type"], "rejected");
        assert_eq!(value["request"], "check-1");
        assert_eq!(value["reason"], token);
    }
    Ok(())
}

#[test]
fn a_revision_outside_the_schema_is_refused() -> TestResult {
    for revision in [0, 9_007_199_254_740_992] {
        let message = UpdatesHostMessage::State { revision, update: Box::new(state()?) };
        assert!(matches!(message.encode(&session()?), Err(UpdatesError::Invalid)));
    }
    Ok(())
}

#[test]
fn page_requests_decode_for_their_session() -> TestResult {
    let envelope = json!({"protocol": "sugarcubes.updates", "version": 1, "session": SESSION});
    let with = |extra: Value| -> Result<Vec<u8>, Box<dyn Error>> {
        let mut value = envelope.clone();
        let fields = value.as_object_mut().ok_or("Envelope is not an object")?;
        fields.extend(extra.as_object().ok_or("Extra is not an object")?.clone());
        Ok(serde_json::to_vec(&value)?)
    };
    assert_eq!(
        UpdatesClientMessage::decode(&with(json!({"type": "watch"}))?)?,
        UpdatesClientMessage::Watch(session()?)
    );
    assert_eq!(
        UpdatesClientMessage::decode(&with(json!({"type": "check", "request": "c-1"}))?)?,
        UpdatesClientMessage::Check {
            session: session()?,
            request: UpdatesRequestName::new("c-1")?
        }
    );
    for (when, timing) in [
        ("now", UpdatesRestartTiming::Now),
        ("after-workflow", UpdatesRestartTiming::AfterWorkflow),
    ] {
        let message = UpdatesClientMessage::decode(&with(
            json!({"type": "restart", "request": "r-1", "when": when}),
        )?)?;
        assert_eq!(
            message,
            UpdatesClientMessage::Restart {
                session: session()?,
                request: UpdatesRequestName::new("r-1")?,
                when: timing,
            }
        );
        assert_eq!(message.session(), &session()?);
    }
    Ok(())
}

#[test]
fn foreign_extended_and_malformed_requests_are_refused() -> TestResult {
    for text in [
        r#"{"protocol":"sugarcubes.updates","version":2,"session":"session-12345678","type":"watch"}"#,
        r#"{"protocol":"sugarcubes.memory","version":1,"session":"session-12345678","type":"watch"}"#,
        r#"{"protocol":"sugarcubes.updates","version":1,"session":"short","type":"watch"}"#,
        r#"{"protocol":"sugarcubes.updates","version":1,"session":"session-12345678","type":"watch","x":1}"#,
        r#"{"protocol":"sugarcubes.updates","version":1,"session":"session-12345678","type":"check","request":"Upper"}"#,
        r#"{"protocol":"sugarcubes.updates","version":1,"session":"session-12345678","type":"restart","request":"r","when":"later"}"#,
        r#"{"protocol":"sugarcubes.updates","version":1,"session":"session-12345678","type":"state"}"#,
    ] {
        assert!(UpdatesClientMessage::decode(text.as_bytes()).is_err(), "{text}");
    }
    let oversized = vec![b' '; MAX_UPDATES_MESSAGE_BYTES + 1];
    assert!(matches!(UpdatesClientMessage::decode(&oversized), Err(UpdatesError::TooLarge)));
    Ok(())
}

#[test]
fn tokens_and_reports_hold_the_schema_bounds() {
    assert!(UpdatesVersion::new(&"1".repeat(64)).is_ok());
    for version in ["", "1.0.0+build", "V1", &"1".repeat(65)] {
        assert!(UpdatesVersion::new(version).is_err(), "{version}");
    }
    assert!(UpdatesRequestName::new(&"a".repeat(49)).is_err());
    let version = || UpdatesVersion::new("1.0.0");
    let report = |causes: Vec<String>| -> Result<UpdatesReport, UpdatesError> {
        UpdatesReport::new(version()?, UpdatesPhase::Check, causes)
    };
    assert!(report(Vec::new()).is_err());
    assert!(report(vec!["x".into(); MAX_REPORT_CAUSES + 1]).is_err());
    assert!(report(vec!["🙂".repeat(MAX_CAUSE_CHARACTERS)]).is_ok());
    assert!(report(vec!["x".repeat(MAX_CAUSE_CHARACTERS + 1)]).is_err());
}

#[test]
fn the_generated_decoder_identifies_the_shared_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = include_str!("../../../../../Sugar/integrations/contracts/updates.schema.json")
        .replace("\r\n", "\n");
    let source =
        include_str!("../../../../../Sugar/integrations/contracts/UpdatesProtocol.generated.rs");
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated updates schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn the_named_bounds_match_the_shared_schema() -> TestResult {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../Sugar/integrations/contracts/updates.schema.json"
    ))?;
    let definitions = &schema["$defs"];
    assert_eq!(definitions["UpdatesReport"]["properties"]["causes"]["maxItems"], MAX_REPORT_CAUSES);
    assert_eq!(
        definitions["UpdatesCause"]["properties"]["message"]["maxLength"],
        MAX_CAUSE_CHARACTERS
    );
    assert_eq!(
        definitions["UpdatesCause"]["properties"]["message"]["x-length-unit"],
        "unicode-codepoints"
    );
    for message in [
        "UpdatesStateMessage",
        "UpdatesAcceptedMessage",
        "UpdatesRejectedMessage",
        "UpdatesWatchMessage",
        "UpdatesCheckMessage",
        "UpdatesRestartMessage",
    ] {
        let envelope = &definitions[message]["properties"];
        assert_eq!(envelope["protocol"]["const"], PROTOCOL, "{message}");
        assert_eq!(envelope["version"]["const"], VERSION, "{message}");
    }
    Ok(())
}
