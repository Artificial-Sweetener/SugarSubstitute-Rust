// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const VECTORS: &str =
    include_str!("../../../../../Sugar/integrations/contracts/session_notice_wire_vectors.json");
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/session-notice.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

const VERSION: u64 = 1;
const SCHEMA_V2: &str =
    include_str!("../../../../../Sugar/integrations/contracts/session-notice-v2.schema.json");

fn encoded(message: &SessionNoticeHostMessage) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&message.encode(&session()?, SessionNoticeVersion::One)?)?)
}

fn encoded_v2(message: &SessionNoticeHostMessage) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&message.encode(&session()?, SessionNoticeVersion::Two)?)?)
}

fn line(text: &str) -> Result<SessionNoticeHostMessage, SessionNoticeError> {
    Ok(SessionNoticeHostMessage::Line {
        notice: NoticeNumber::new(2)?,
        tone: NoticeTone::Warn,
        text: text.into(),
        lasting: true,
    })
}

#[test]
fn a_line_is_sent_only_to_a_page_that_speaks_version_two() -> TestResult {
    assert_eq!(
        encoded_v2(&line("Saving your workspace…")?)?,
        json!({"protocol": "sugarcubes.session-notice", "version": 2, "session": SESSION,
            "type": "line", "notice": 2, "tone": "warn", "text": "Saving your workspace…",
            "lasting": true})
    );
    assert!(matches!(
        line("Saving your workspace…")?.encode(&session()?, SessionNoticeVersion::One),
        Err(SessionNoticeError::Invalid)
    ));
    // The other notices read the same in version 2, under its number.
    assert_eq!(
        encoded_v2(&SessionNoticeHostMessage::None)?,
        json!({"protocol": "sugarcubes.session-notice", "version": 2, "session": SESSION,
            "type": "none"})
    );
    Ok(())
}

#[test]
fn a_blank_or_overlong_line_is_never_sent() -> TestResult {
    for text in [String::new(), "  ".into(), "é".repeat(MAX_LINE_CHARACTERS + 1)] {
        assert!(matches!(
            line(&text)?.encode(&session()?, SessionNoticeVersion::Two),
            Err(SessionNoticeError::Invalid)
        ));
    }
    // The bound counts characters, and the longest line still fits the byte bound.
    let longest = line(&"é".repeat(MAX_LINE_CHARACTERS))?;
    assert!(longest.encode(&session()?, SessionNoticeVersion::Two).is_ok());
    let schema: Value = serde_json::from_str(SCHEMA_V2)?;
    assert_eq!(
        schema["$defs"]["SessionNoticeLineMessage"]["properties"]["text"]["maxLength"],
        MAX_LINE_CHARACTERS
    );
    Ok(())
}

#[test]
fn a_version_two_page_is_recognised_by_its_watch() -> TestResult {
    let watch = json!({"protocol": "sugarcubes.session-notice", "version": 2,
        "session": SESSION, "type": "watch"});
    assert_eq!(
        SessionNoticeClientMessage::decode(&serde_json::to_vec(&watch)?)?,
        SessionNoticeClientMessage::Watch {
            session: session()?,
            version: SessionNoticeVersion::Two
        }
    );
    let shown = json!({"protocol": "sugarcubes.session-notice", "version": 2,
        "session": SESSION, "type": "shown", "notice": 2});
    assert_eq!(
        SessionNoticeClientMessage::decode(&serde_json::to_vec(&shown)?)?,
        SessionNoticeClientMessage::Shown { session: session()?, notice: NoticeNumber::new(2)? }
    );
    let unknown = json!({"protocol": "sugarcubes.session-notice", "version": 3,
        "session": SESSION, "type": "watch"});
    assert!(SessionNoticeClientMessage::decode(&serde_json::to_vec(&unknown)?).is_err());
    Ok(())
}

#[test]
fn the_version_two_decoder_identifies_its_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA_V2.replace("\r\n", "\n");
    let source = include_str!(
        "../../../../../Sugar/integrations/contracts/SessionNoticeV2Protocol.generated.rs"
    );
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated session notice schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn every_host_message_encodes_every_schema_field() -> TestResult {
    assert_eq!(
        encoded(&SessionNoticeHostMessage::NotSaved {
            notice: NoticeNumber::new(3)?,
            workspace: true,
            inputs: false,
        })?,
        json!({"protocol": "sugarcubes.session-notice", "version": 1, "session": SESSION,
            "type": "not-saved", "notice": 3, "workspace": true, "inputs": false})
    );
    assert_eq!(
        encoded(&SessionNoticeHostMessage::None)?,
        json!({"protocol": "sugarcubes.session-notice", "version": 1, "session": SESSION,
            "type": "none"})
    );
    Ok(())
}

#[test]
fn a_notice_that_names_nothing_unsaved_is_never_sent() -> TestResult {
    let nothing = SessionNoticeHostMessage::NotSaved {
        notice: NoticeNumber::new(1)?,
        workspace: false,
        inputs: false,
    };
    assert!(matches!(
        nothing.encode(&session()?, SessionNoticeVersion::One),
        Err(SessionNoticeError::Invalid)
    ));
    Ok(())
}

#[test]
fn notice_numbers_stay_within_what_the_page_counts_exactly() {
    assert!(NoticeNumber::new(0).is_err());
    assert!(NoticeNumber::new(MAX_NOTICE_NUMBER).is_ok());
    assert!(NoticeNumber::new(MAX_NOTICE_NUMBER + 1).is_err());
}

#[test]
fn page_messages_decode_to_their_session_and_notice() -> TestResult {
    let watch = json!({"protocol": "sugarcubes.session-notice", "version": 1,
        "session": SESSION, "type": "watch"});
    let decoded = SessionNoticeClientMessage::decode(&serde_json::to_vec(&watch)?)?;
    assert_eq!(
        decoded,
        SessionNoticeClientMessage::Watch {
            session: session()?,
            version: SessionNoticeVersion::One
        }
    );
    assert_eq!(decoded.session(), &session()?);
    let shown = json!({"protocol": "sugarcubes.session-notice", "version": 1,
        "session": SESSION, "type": "shown", "notice": 7});
    assert_eq!(
        SessionNoticeClientMessage::decode(&serde_json::to_vec(&shown)?)?,
        SessionNoticeClientMessage::Shown { session: session()?, notice: NoticeNumber::new(7)? }
    );
    let oversized = vec![b' '; MAX_SESSION_NOTICE_MESSAGE_BYTES + 1];
    assert!(matches!(
        SessionNoticeClientMessage::decode(&oversized),
        Err(SessionNoticeError::TooLarge)
    ));
    Ok(())
}

#[test]
fn the_shared_vectors_hold_in_both_directions() -> TestResult {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS)?;
    assert!(!vectors.is_empty());
    for vector in &vectors {
        let label = vector["label"].as_str().ok_or("Vector has no label")?;
        let valid = vector["valid"].as_bool().ok_or("Vector has no verdict")?;
        let bytes = serde_json::to_vec(&vector["value"])?;
        let host = matches!(vector["value"]["type"].as_str(), Some("not-saved" | "none"));
        let as_host = serde_json::from_slice::<wire::SessionNoticeHostMessage>(&bytes)
            .is_ok_and(|message| message.valid());
        // The vectors are version 1's; a message only version 2 admits is not one of them.
        let as_client = serde_json::from_slice::<wire::SessionNoticeClientMessage>(&bytes)
            .is_ok_and(|message| message.valid());
        assert_eq!((label, as_host), (label, valid && host));
        assert_eq!((label, as_client), (label, valid && !host));
    }
    Ok(())
}

#[test]
fn the_generated_decoder_identifies_the_shared_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source = include_str!(
        "../../../../../Sugar/integrations/contracts/SessionNoticeProtocol.generated.rs"
    );
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated session notice schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn the_named_bounds_match_the_shared_schema() -> TestResult {
    let schema: Value = serde_json::from_str(SCHEMA)?;
    let definitions = &schema["$defs"];
    for message in ["SessionNoticeNotSavedMessage", "SessionNoticeShownMessage"] {
        assert_eq!(
            definitions[message]["properties"]["notice"]["maximum"], MAX_NOTICE_NUMBER,
            "{message}"
        );
    }
    for message in [
        "SessionNoticeNotSavedMessage",
        "SessionNoticeNoneMessage",
        "SessionNoticeWatchMessage",
        "SessionNoticeShownMessage",
    ] {
        let envelope = &definitions[message]["properties"];
        assert_eq!(envelope["protocol"]["const"], PROTOCOL, "{message}");
        assert_eq!(envelope["version"]["const"], VERSION, "{message}");
    }
    Ok(())
}
