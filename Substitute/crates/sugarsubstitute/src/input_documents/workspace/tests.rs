// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the native metadata consumer against the actual SugarCubes decoder vectors.
use super::*;
#[test]
fn supported_input_metadata_matches_all_cross_runtime_vectors() -> Result<(), Box<dyn Error>> {
    let vectors: serde_json::Value = serde_json::from_str(include_str!("vectors.json"))?;
    let cases = vectors["cases"].as_array().ok_or("Missing metadata vectors")?;
    assert_eq!(cases.len(), 24);
    for case in cases {
        let root = serde_json::json!({"extra":{"sugarcubes_input_canvas":case["record"]}});
        let admitted = selection(&root).is_ok_and(|value| value.is_some());
        assert_eq!(
            admitted,
            case["valid"].as_bool().ok_or("Missing vector verdict")?,
            "{}",
            case["name"]
        );
    }
    Ok(())
}
