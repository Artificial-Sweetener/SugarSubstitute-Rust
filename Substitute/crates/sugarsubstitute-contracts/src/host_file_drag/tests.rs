// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/file-drag.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn entered(names: &[&str]) -> FileDragHostMessage {
    FileDragHostMessage::Entered { names: names.iter().map(|&name| name.to_owned()).collect() }
}

fn encoded(message: &FileDragHostMessage) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&message.encode(&session()?)?)?)
}

#[test]
fn every_host_message_encodes_every_schema_field() -> TestResult {
    assert_eq!(
        encoded(&entered(&["Portrait.json", "render.png"]))?,
        json!({"protocol": "sugarcubes.file-drag", "version": 1, "session": SESSION,
            "type": "entered", "files": [{"name": "Portrait.json"}, {"name": "render.png"}]})
    );
    assert_eq!(
        encoded(&FileDragHostMessage::Left)?,
        json!({"protocol": "sugarcubes.file-drag", "version": 1, "session": SESSION,
            "type": "left"})
    );
    Ok(())
}

#[test]
fn only_bounded_file_names_are_sent() -> TestResult {
    let longest = "n".repeat(MAX_FILE_NAME_UNITS);
    assert!(entered(&[&longest]).encode(&session()?).is_ok());
    assert!(entered(&["a.json"; MAX_DRAGGED_FILES]).encode(&session()?).is_ok());
    let too_long = "n".repeat(MAX_FILE_NAME_UNITS + 1);
    for refused in [
        entered(&[]),
        entered(&["a.json"; MAX_DRAGGED_FILES + 1]),
        entered(&[""]),
        entered(&[&too_long]),
        entered(&[r"C:\work\a.json"]),
        entered(&["work/a.json"]),
    ] {
        assert!(matches!(refused.encode(&session()?), Err(FileDragError::Invalid)), "{refused:?}");
    }
    Ok(())
}

#[test]
fn the_largest_message_fits_its_byte_bound() -> TestResult {
    // Each UTF-16 unit of a name takes at most three UTF-8 bytes.
    let widest = "\u{ffff}".repeat(MAX_FILE_NAME_UNITS);
    let names: Vec<&str> = vec![&widest; MAX_DRAGGED_FILES];
    assert!(entered(&names).encode(&session()?)?.len() <= MAX_FILE_DRAG_MESSAGE_BYTES);
    Ok(())
}

#[test]
fn the_generated_shapes_identify_their_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source =
        include_str!("../../../../../Sugar/integrations/contracts/FileDragProtocol.generated.rs");
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated file drag schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}
