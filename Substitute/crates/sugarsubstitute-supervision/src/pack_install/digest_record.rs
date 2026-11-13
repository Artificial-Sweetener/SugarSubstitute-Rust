// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record the digest of every tracked file so local edits are found offline.
//!
//! `.tracking-digests.json` sits beside `.tracking` and is written whenever this format
//! installs a pack. It binds itself to the exact `.tracking` bytes and the installed version:
//! if anything else (such as ComfyUI Manager) reinstalls the pack, the record no longer
//! matches and is ignored rather than trusted. Version 1 is
//! `{"schema": 1, "version": "x.y.z", "tracking": "<sha256 of .tracking>",
//! "files": {"<tracked file path>": "<sha256>"}}`, rendered exactly as SugarCubes renders it
//! (two-space indentation, sorted file keys, ASCII-escaped strings, no trailing newline).

use super::entry::EntryName;
use std::collections::{BTreeMap, BTreeSet};

/// The digest-record version the shared install-format catalog records.
pub const DIGEST_RECORD_VERSION: u64 = 1;
/// The largest digest record read.
pub const MAX_DIGEST_RECORD_BYTES: usize = 8 * 1024 * 1024;

/// Tracked file path to lowercase SHA-256.
pub type FileDigests = BTreeMap<String, String>;

/// Return the recorded digests, or `None` when the record does not describe this install.
///
/// A record applies only to the `.tracking` bytes and version it names and must list exactly
/// the tracked files with well-formed digests.
pub fn parse_digest_record(
    content: &[u8],
    version: &str,
    tracking_sha256: &str,
    entries: &[EntryName],
) -> Option<FileDigests> {
    if content.len() > MAX_DIGEST_RECORD_BYTES {
        return None;
    }
    let payload: serde_json::Value = serde_json::from_slice(content).ok()?;
    let object = payload.as_object()?;
    if object.get("schema")?.as_u64()? != DIGEST_RECORD_VERSION
        || object.get("version")?.as_str()? != version
        || object.get("tracking")?.as_str()? != tracking_sha256
    {
        return None;
    }
    let files = object.get("files")?.as_object()?;
    let tracked: BTreeSet<&str> =
        entries.iter().filter(|entry| !entry.is_directory()).map(EntryName::path).collect();
    if files.len() != tracked.len() || !files.keys().all(|path| tracked.contains(path.as_str())) {
        return None;
    }
    files
        .iter()
        .map(|(path, digest)| {
            let digest = digest.as_str().filter(|digest| is_sha256(digest))?;
            Some((path.clone(), digest.to_owned()))
        })
        .collect()
}

/// Return whether a text is a lowercase hexadecimal SHA-256.
pub(crate) fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Render the record for one installed release, byte-identical to SugarCubes' writer.
pub fn render_digest_record(version: &str, tracking_sha256: &str, files: &FileDigests) -> String {
    let mut text = String::from("{\n");
    text.push_str(&format!("  \"schema\": {DIGEST_RECORD_VERSION},\n"));
    text.push_str(&format!("  \"version\": {},\n", quoted(version)));
    text.push_str(&format!("  \"tracking\": {},\n", quoted(tracking_sha256)));
    if files.is_empty() {
        text.push_str("  \"files\": {}\n}");
        return text;
    }
    text.push_str("  \"files\": {\n");
    let lines: Vec<String> = files
        .iter()
        .map(|(path, digest)| format!("    {}: {}", quoted(path), quoted(digest)))
        .collect();
    text.push_str(&lines.join(",\n"));
    text.push_str("\n  }\n}");
    text
}

/// Quote a string as Python's `json.dumps` does with `ensure_ascii`.
fn quoted(value: &str) -> String {
    let mut text = String::with_capacity(value.len() + 2);
    text.push('"');
    for character in value.chars() {
        match character {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            '\u{8}' => text.push_str("\\b"),
            '\u{c}' => text.push_str("\\f"),
            ' '..='~' => text.push(character),
            other => {
                let mut units = [0_u16; 2];
                for unit in other.encode_utf16(&mut units) {
                    text.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    text.push('"');
    text
}
