// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the host's install format satisfies every shared language-neutral vector.
//!
//! The vectors are SugarCubes' own file, read in place; the Python suite drives the same
//! file. A group added there fails here until a test drives every vector of it.

mod decisions;
mod fixture;
mod host;
mod layout;

use super::*;
use fixture::{Failure, field, list, text, texts};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const VECTORS: &str =
    include_str!("../../../../../Sugar/tests/pack_install/support/registry_install_vectors.json");

type TestResult = std::result::Result<(), Failure>;

/// Every vector group and the Rust test that drives each of its vectors.
const RUST_VECTOR_GROUPS: [(&str, &str); 12] = [
    ("formats", "format_versions_match_the_shared_catalog"),
    ("trackingParse", "tracking_parse_vectors"),
    ("trackingRender", "tracking_render_vectors"),
    ("archives", "archive_vectors"),
    ("archiveLimits", "archive_limit_vectors"),
    ("ownership", "ownership_vectors"),
    ("localEdits", "local_edit_vectors"),
    ("replacements", "replacement_vectors"),
    ("digestManifests", "digest_manifest_vectors"),
    ("sourceSelection", "source_selection_vectors"),
    ("updateRefusals", "update_refusal_vectors"),
    ("rollbackTriggers", "rollback_trigger_vectors"),
];

fn vectors() -> std::result::Result<Value, Failure> {
    Ok(serde_json::from_str(VECTORS)?)
}

fn group(name: &str) -> std::result::Result<Vec<Value>, Failure> {
    Ok(list(&vectors()?, name)?.clone())
}

fn code(error: &PackInstallError) -> &'static str {
    error.code().map_or("io", ErrorCode::as_str)
}

fn sha256(bytes: &[u8]) -> String {
    ownership::sha256_hex(bytes)
}

#[test]
fn every_vector_group_is_driven_and_every_name_is_unique() -> TestResult {
    let document = vectors()?;
    let groups: BTreeSet<&str> = document
        .as_object()
        .ok_or("vectors are not an object")?
        .keys()
        .map(String::as_str)
        .filter(|name| !matches!(*name, "schema" | "description"))
        .collect();
    let driven: BTreeSet<&str> = RUST_VECTOR_GROUPS.iter().map(|(name, _)| *name).collect();
    assert_eq!(groups, driven);
    for name in groups {
        let value = field(&document, name)?;
        let cases = value.as_array().or_else(|| value.get("cases").and_then(Value::as_array));
        let names: Vec<&str> = cases
            .into_iter()
            .flatten()
            .map(|case| case.get("name").and_then(Value::as_str).unwrap_or_default())
            .collect();
        assert_eq!(names.len(), names.iter().collect::<BTreeSet<_>>().len(), "{name}");
    }
    Ok(())
}

#[test]
fn format_versions_match_the_shared_catalog() -> TestResult {
    let document = vectors()?;
    let formats = field(&document, "formats")?;
    let tracking = field(formats, "tracking")?;
    let digests = field(formats, "digestRecord")?;
    assert_eq!(text(tracking, "file")?, TRACKING_FILE_NAME);
    assert_eq!(field(tracking, "version")?.as_u64(), Some(u64::from(TRACKING_FORMAT_VERSION)));
    assert_eq!(text(digests, "file")?, DIGEST_RECORD_NAME);
    assert_eq!(field(digests, "version")?.as_u64(), Some(DIGEST_RECORD_VERSION));
    Ok(())
}

#[test]
fn tracking_parse_vectors() -> TestResult {
    for vector in group("trackingParse")? {
        let name = text(&vector, "name")?;
        let expect = field(&vector, "expect")?;
        let parsed = parse_tracking(text(&vector, "text")?.as_bytes());
        match expect.get("error").and_then(Value::as_str) {
            Some(error) => match parsed {
                Err(cause) => assert_eq!(code(&cause), error, "{name}"),
                Ok(entries) => return Err(format!("{name}: accepted {entries:?}").into()),
            },
            None => {
                let entries: Vec<String> =
                    parsed?.iter().map(|entry| entry.text().to_owned()).collect();
                assert_eq!(entries, texts(expect, "entries")?, "{name}");
            }
        }
    }
    Ok(())
}

#[test]
fn tracking_render_vectors() -> TestResult {
    for vector in group("trackingRender")? {
        let entries = texts(&vector, "entries")?
            .iter()
            .map(|entry| parse_entry(entry))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(render_tracking(&entries), text(&vector, "lfText")?);
        let on_disk = if cfg!(windows) { "crlfText" } else { "lfText" };
        assert_eq!(tracking_bytes(&entries), text(&vector, on_disk)?.as_bytes());
        assert_eq!(parse_tracking(&tracking_bytes(&entries))?, entries);
    }
    Ok(())
}

#[test]
fn local_edit_vectors() -> TestResult {
    for vector in group("localEdits")? {
        let name = text(&vector, "name")?;
        let installed: BTreeMap<String, Option<String>> = field(&vector, "installed")?
            .as_object()
            .ok_or("installed is not a map")?
            .iter()
            .map(|(path, content)| {
                (path.clone(), content.as_str().map(|text| sha256(text.as_bytes())))
            })
            .collect();
        let published: FileDigests = field(&vector, "published")?
            .as_object()
            .ok_or("published is not a map")?
            .iter()
            .map(|(path, content)| {
                (path.clone(), sha256(content.as_str().unwrap_or_default().as_bytes()))
            })
            .collect();
        assert_eq!(
            changed_tracked_files(&installed, &published),
            texts(&vector, "expectChanged")?,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn digest_manifest_vectors() -> TestResult {
    let manifests = field(&vectors()?, "digestManifests")?.clone();
    let tracking_text = text(&manifests, "trackingText")?;
    let entries = parse_tracking(tracking_text.as_bytes())?;
    let version = text(&manifests, "version")?;
    for vector in list(&manifests, "cases")? {
        let name = text(vector, "name")?;
        let content = serde_json::to_vec(field(vector, "manifest")?)?;
        let recorded =
            parse_digest_record(&content, version, &sha256(tracking_text.as_bytes()), &entries);
        let expect = field(vector, "expect")?;
        if field(expect, "applies")?.as_bool() == Some(true) {
            let files: FileDigests = serde_json::from_value(field(expect, "files")?.clone())?;
            assert_eq!(recorded, Some(files), "{name}");
        } else {
            assert_eq!(recorded, None, "{name}");
        }
    }
    let first = field(&list(&manifests, "cases")?[0], "manifest")?;
    let files: FileDigests = serde_json::from_value(field(first, "files")?.clone())?;
    let rendered = render_digest_record(version, text(first, "tracking")?, &files);
    assert_eq!(rendered, text(&manifests, "rendered")?);
    Ok(())
}
