// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/update-report.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn envelope(extra: Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut value =
        json!({"protocol": "sugarcubes.update-report", "version": 1, "session": SESSION});
    if let (Some(target), Some(source)) = (value.as_object_mut(), extra.as_object()) {
        target.extend(source.clone());
    }
    serde_json::to_vec(&value)
}

#[test]
fn every_state_encodes_every_schema_field() -> TestResult {
    for available in [false, true] {
        let encoded =
            UpdateReportHostMessage::State { revision: 7, available }.encode(&session()?)?;
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded)?,
            json!({"protocol": "sugarcubes.update-report", "version": 1, "session": SESSION,
                "type": "state", "revision": 7, "available": available})
        );
    }
    let beyond = UpdateReportHostMessage::State {
        revision: MAX_UPDATE_REPORT_REVISION + 1,
        available: true,
    };
    assert!(matches!(beyond.encode(&session()?), Err(UpdateReportError::Invalid)));
    Ok(())
}

#[test]
fn the_page_watches_and_asks_for_the_report_of_the_state_it_saw() -> TestResult {
    assert_eq!(
        UpdateReportClientMessage::decode(&envelope(json!({"type": "watch"}))?)?,
        UpdateReportClientMessage::Watch { session: session()? }
    );
    assert_eq!(
        UpdateReportClientMessage::decode(&envelope(json!({"type": "open", "revision": 3}))?)?,
        UpdateReportClientMessage::Open { session: session()?, revision: 3 }
    );
    for refused in [
        json!({"type": "open"}),
        json!({"type": "open", "revision": -1}),
        json!({"type": "open", "revision": 3, "export": true}),
        json!({"type": "state", "revision": 3, "available": true}),
        json!({"type": "watch", "version": 2}),
        json!({"type": "watch", "session": "short"}),
    ] {
        assert!(
            UpdateReportClientMessage::decode(&envelope(refused.clone())?).is_err(),
            "{refused}"
        );
    }
    assert!(matches!(
        UpdateReportClientMessage::decode(&vec![b' '; MAX_UPDATE_REPORT_MESSAGE_BYTES + 1]),
        Err(UpdateReportError::TooLarge)
    ));
    Ok(())
}

#[test]
fn the_generated_shapes_identify_their_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source = include_str!(
        "../../../../../Sugar/integrations/contracts/UpdateReportProtocol.generated.rs"
    );
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated update report schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}
