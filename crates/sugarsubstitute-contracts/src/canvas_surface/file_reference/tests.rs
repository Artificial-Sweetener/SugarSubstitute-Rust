// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match shared web vectors and reject ambiguous file-reference wire values.

use super::*;
use serde_json::Value;

#[test]
fn shared_file_reference_vectors_match_web_validation() -> Result<(), Box<dyn Error>> {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-canvas/file_vectors.json"
    )))?;
    for case in vectors {
        let name = case.get("name").and_then(Value::as_str).ok_or("Missing vector name")?;
        let expected = case.get("valid").and_then(Value::as_bool).ok_or("Missing vector result")?;
        let value = case.get("value").ok_or("Missing vector value")?;
        assert_eq!(
            CanvasFileReference::decode(&serde_json::to_vec(value)?).is_ok(),
            expected,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn reference_preserves_unicode_and_rejects_overlong_or_ambiguous_json() -> Result<(), Box<dyn Error>>
{
    let value =
        serde_json::json!({"filename":"🌈".repeat(4096),"subfolder":"batch","type":"output"});
    let file = CanvasFileReference::decode(&serde_json::to_vec(&value)?)?;
    assert_eq!(file.filename(), "🌈".repeat(4096));
    assert_eq!(file.subfolder(), "batch");
    assert_eq!(file.storage(), CanvasFileStorage::Output);
    let long = serde_json::json!({"filename":"🌈".repeat(4097),"subfolder":"","type":"output"});
    assert!(matches!(
        CanvasFileReference::decode(&serde_json::to_vec(&long)?),
        Err(CanvasFileError::OutOfBounds)
    ));
    assert!(matches!(
        CanvasFileReference::decode(&vec![b' '; MAX_FILE_REFERENCE_BYTES + 1]),
        Err(CanvasFileError::TooLarge)
    ));
    assert!(matches!(
        CanvasFileReference::decode(
            br#"{"filename":"a","filename":"b","subfolder":"","type":"output"}"#
        ),
        Err(CanvasFileError::Malformed(_))
    ));
    assert!(matches!(
        CanvasFileReference::decode(br#"{"filename":"\ud800","subfolder":"","type":"output"}"#),
        Err(CanvasFileError::Malformed(_))
    ));
    Ok(())
}
