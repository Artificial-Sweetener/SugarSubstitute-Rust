// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep complete syntax and typed metadata bounded without retaining a generic JSON tree.
use super::*;
type TestResult = Result<(), Box<dyn Error>>;
#[test]
fn nested_unknown_cohorts_are_ignored_after_complete_bounded_syntax() -> TestResult {
    let baseline = serde_json::to_string(&system(&["main.py"]))?;
    let cohort = format!("[{}]", vec!["0"; MAX_EXTENSION_PATHS].join(","));
    let stats =
        format!("{{\"supplemental\":[{}],{}", vec![cohort.clone(); 7].join(","), &baseline[1..]);
    assert!(stats.len() < MAX_SYSTEM_STATS_BYTES);
    decode_system_stats(stats.as_bytes())?;
    let large = format!("[{}]", vec![cohort; 100].join(","));
    assert!(large.len() > 800_000 && large.len() < MAX_EXTENSIONS_BYTES);
    json::decode(large.as_bytes(), MAX_EXTENSIONS_BYTES)?;
    assert!(decode_extensions(large.as_bytes()).is_err());
    Ok(())
}
#[test]
fn exact_original_map_array_and_depth_limits_survive_without_dom() -> TestResult {
    let map = |count| {
        format!("{{{}}}", (0..count).map(|i| format!("\"key{i}\":0")).collect::<Vec<_>>().join(","))
    };
    json::decode(map(128).as_bytes(), MAX_EXTENSIONS_BYTES)?;
    assert!(json::decode(map(129).as_bytes(), MAX_EXTENSIONS_BYTES).is_err());
    let array = |count| format!("[{}]", vec!["0"; count].join(","));
    json::decode(array(4096).as_bytes(), MAX_EXTENSIONS_BYTES)?;
    assert!(json::decode(array(4097).as_bytes(), MAX_EXTENSIONS_BYTES).is_err());
    let nested = |depth| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    json::decode(nested(32).as_bytes(), MAX_EXTENSIONS_BYTES)?;
    assert!(json::decode(nested(33).as_bytes(), MAX_EXTENSIONS_BYTES).is_err());
    for raw in [
        br#"{"a":1,"\u0061":2}"#.as_slice(),
        br#"{"unknown":{"schema_version":1}}"#,
        br#"{"unknown":{"a":1,"a":2}}"#,
    ] {
        assert!(json::decode(raw, MAX_EXTENSIONS_BYTES).is_err());
    }
    Ok(())
}
#[test]
fn maximum_raw_and_escaped_wrong_text_keeps_small_genuine_json_cause() -> TestResult {
    let mut raw = vec![0x7f; MAX_EXTENSIONS_BYTES];
    raw[0] = b'"';
    raw[MAX_EXTENSIONS_BYTES - 1] = b'"';
    let escaped = format!("\"{}\"", "\\u007f".repeat((MAX_EXTENSIONS_BYTES - 2) / 6));
    for bytes in [raw.as_slice(), escaped.as_bytes()] {
        let ReadinessError::Json(error) =
            decode_extensions(bytes).err().ok_or("Expected typed error")?
        else {
            return Err("Missing genuine JSON error".into());
        };
        assert!(error.is_data());
        assert!(error.column() > 0);
        assert!(error.to_string().len() < 100);
    }
    Ok(())
}
#[test]
fn maximum_served_inventory_has_exact_fixed_headers_and_owned_text_capacity() -> TestResult {
    let paths: Vec<String> =
        (0..4096).map(|i| format!("/extensions/SugarCubes/{i:04}{}.js", "x".repeat(220))).collect();
    let raw = serde_json::to_vec(&paths)?;
    assert!(raw.len() <= MAX_EXTENSIONS_BYTES);
    let result = decode_extensions(&raw)?;
    assert_eq!(result.paths.capacity(), MAX_EXTENSION_PATHS);
    assert_eq!(result.paths, paths);
    assert!(result.paths.iter().all(|text| text.capacity() == text.len()));
    let heap = result.paths.capacity() * size_of::<String>()
        + result.paths.iter().map(String::capacity).sum::<usize>();
    assert!(heap <= MAX_EXTENSIONS_BYTES + MAX_EXTENSION_PATHS * size_of::<String>());
    Ok(())
}
