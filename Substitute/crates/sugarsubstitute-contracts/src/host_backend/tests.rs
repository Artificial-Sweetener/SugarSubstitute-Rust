// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/backend.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn envelope(extra: Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut value = json!({"protocol": "sugarcubes.backend", "version": 1, "session": SESSION});
    if let (Some(target), Some(source)) = (value.as_object_mut(), extra.as_object()) {
        target.extend(source.clone());
    }
    serde_json::to_vec(&value)
}

#[test]
fn every_state_encodes_every_schema_field() -> TestResult {
    for (status, word) in [
        (BackendStatus::Running, "running"),
        (BackendStatus::Stopped, "stopped"),
        (BackendStatus::RetryReady, "retry-ready"),
        (BackendStatus::Retrying, "retrying"),
    ] {
        let encoded = BackendHostMessage::State { revision: 7, status }.encode(&session()?)?;
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded)?,
            json!({"protocol": "sugarcubes.backend", "version": 1, "session": SESSION,
                "type": "state", "revision": 7, "status": word})
        );
    }
    let beyond = BackendHostMessage::State {
        revision: MAX_BACKEND_REVISION + 1,
        status: BackendStatus::Running,
    };
    assert!(matches!(beyond.encode(&session()?), Err(BackendError::Invalid)));
    Ok(())
}

#[test]
fn the_page_watches_and_asks_for_a_restart_of_the_state_it_saw() -> TestResult {
    assert_eq!(
        BackendClientMessage::decode(&envelope(json!({"type": "watch"}))?)?,
        BackendClientMessage::Watch { session: session()? }
    );
    assert_eq!(
        BackendClientMessage::decode(&envelope(json!({"type": "retry", "revision": 3}))?)?,
        BackendClientMessage::Retry { session: session()?, revision: 3 }
    );
    for refused in [
        json!({"type": "retry"}),
        json!({"type": "retry", "revision": 0}),
        json!({"type": "retry", "revision": 3, "force": true}),
        json!({"type": "state", "revision": 3, "status": "running"}),
        json!({"type": "watch", "version": 2}),
        json!({"type": "watch", "session": "short"}),
    ] {
        assert!(BackendClientMessage::decode(&envelope(refused.clone())?).is_err(), "{refused}");
    }
    assert!(matches!(
        BackendClientMessage::decode(&vec![b' '; MAX_BACKEND_MESSAGE_BYTES + 1]),
        Err(BackendError::TooLarge)
    ));
    Ok(())
}

#[test]
fn the_generated_shapes_identify_their_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source =
        include_str!("../../../../../Sugar/integrations/contracts/BackendProtocol.generated.rs");
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated backend schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}
