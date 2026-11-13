// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The same package rejects hostile inventories on every host platform.
use super::*;
use serde_json::{Value, json};
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
fn entry(name: &str, bytes: u64) -> Value {
    json!({"path":name,"bytes":bytes,"sha256":"a".repeat(64)})
}
fn inventory(files: Vec<Value>) -> Value {
    json!({"schema_version":1,"release_id":"native-2","files":files})
}
fn decode(value: &Value) -> Result<Manifest, Rejection> {
    Manifest::decode(value.to_string().as_bytes())
}

#[test]
fn portable_names_reject_normalization_and_device_aliases_before_filesystem_access() {
    for path in [
        "../x",
        "a/./x",
        "a/../x",
        "a//x",
        "a\\x",
        "a:stream",
        "NUL.txt",
        "NUL .txt",
        "x.",
        "x ",
        "/x",
        "a/?",
        "COM¹.txt",
        "CONOUT$",
        "app\0file",
    ] {
        assert_eq!(relative_path(path), Err(Rejection::Path), "{path}");
    }
    assert!(relative_path("sugarcubes/web/糖.js").is_ok());
    assert!(relative_path(&"a".repeat(256)).is_err());
}

#[test]
fn inventory_rejects_duplicates_file_ancestors_unknown_roles_and_malformed_identity() -> TestResult
{
    for files in [
        vec![entry("app/one", 1), entry("app/ONE", 1)],
        vec![entry("app/one", 1), entry("app/one/two", 1)],
        vec![entry("unknown/file", 1)],
    ] {
        assert!(matches!(decode(&inventory(files)), Err(Rejection::Inventory)));
    }
    let mut value = inventory(vec![entry("app/file", 1)]);
    assert_eq!(decode(&value)?.release(), "native-2");
    assert_eq!(decode(&value)?.entries().len(), 1);
    value["files"][0]["sha256"] = "A".repeat(64).into();
    assert!(decode(&value).is_err());
    value["files"][0]["sha256"] = "a".repeat(64).into();
    value["release_id"] = "../../other".into();
    assert!(decode(&value).is_err());
    Ok(())
}

#[test]
fn package_budgets_reject_aggregate_expansion_even_when_each_file_is_legal() {
    let files: Vec<_> =
        (0..9).map(|n| entry(&format!("app/file{n}"), 2 * 1024 * 1024 * 1024)).collect();
    assert!(matches!(decode(&inventory(files)), Err(Rejection::Bound)));
    assert!(matches!(
        decode(&inventory(vec![entry("app/file", 2 * 1024 * 1024 * 1024 + 1)])),
        Err(Rejection::Bound)
    ));
    assert!(matches!(Manifest::decode(&vec![b' '; MAX_MANIFEST_BYTES + 1]), Err(Rejection::Bound)));
    assert!(matches!(decode(&inventory(Vec::new())), Err(Rejection::Bound)));
}

#[test]
fn unknown_versions_and_fields_cannot_silently_change_package_meaning() {
    let mut value = inventory(vec![entry("app/file", 1)]);
    value["schema_version"] = 2.into();
    assert!(matches!(decode(&value), Err(Rejection::Format)));
    value["schema_version"] = 1.into();
    value["execute"] = "script".into();
    assert!(matches!(decode(&value), Err(Rejection::Format)));
    value = inventory(vec![entry("app/file", 1)]);
    value["files"][0]["symlink"] = "../outside".into();
    assert!(matches!(decode(&value), Err(Rejection::Format)));
}
