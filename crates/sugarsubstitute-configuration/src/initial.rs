// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initial installed startup values; existing settings are preserved by the storage owner.
use crate::startup::{MAX_STARTUP_CONFIGURATION_BYTES, parse_startup_configuration};
use std::{io, path::Path};

/// Produce startup for an installation-owned ComfyUI environment and explicit data/output paths.
/// Port 8188, profile `default`, dark appearance and full motion are initial values;
/// startup requires every persisted field explicitly and never reapplies these defaults.
/// `data` holds ComfyUI's mutable base and its user/temporary children independently of source.
/// # Errors
/// Rejects invalid/non-Unicode paths, locale or a document above the startup byte bound.
pub fn installation_owned(
    root: &Path,
    interpreter: &Path,
    data: &Path,
    outputs: &Path,
    locale: &str,
) -> io::Result<String> {
    sugarsubstitute_contracts::configuration::LaunchConfiguration::validate_path(data)
        .map_err(io::Error::other)?;
    launch_document(
        root,
        interpreter,
        vec![
            "--base-directory".into(),
            unicode(data)?,
            "--user-directory".into(),
            unicode(&data.join("user"))?,
            "--temp-directory".into(),
            unicode(&data.join("temp"))?,
            "--output-directory".into(),
            unicode(outputs)?,
        ],
        locale,
    )
}

/// Produce startup for an explicitly selected external ComfyUI, preserving its ordinary
/// base/user/temporary locations. The selected output override remains explicit.
/// # Errors
/// Rejects invalid/non-Unicode paths, locale or a document above the startup byte bound.
pub fn external(
    root: &Path,
    interpreter: &Path,
    outputs: &Path,
    locale: &str,
) -> io::Result<String> {
    launch_document(root, interpreter, vec!["--output-directory".into(), unicode(outputs)?], locale)
}

/// Encode the one implemented startup schema and validate through its runtime parser.
fn launch_document(
    root: &Path,
    interpreter: &Path,
    arguments: Vec<String>,
    locale: &str,
) -> io::Result<String> {
    let document = toml::to_string(&serde_json::json!({
        "schema_version": 1, "mode": "launch", "port": 8188, "profile": "default",
        "locale": locale, "theme": "dark", "reduced_motion": false,
        "launch": { "root": unicode(root)?, "interpreter": unicode(interpreter)?, "arguments": arguments },
    })).map_err(io::Error::other)?;
    parse_startup_configuration(document.as_bytes()).map_err(io::Error::other)?;
    Ok(document)
}

/// Preserve the selected path's exact Unicode spelling in the persisted document.
fn unicode(path: &Path) -> io::Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Installed startup path is not Unicode")
    })
}

/// Check the historical preservation contract without reserializing user-authored text.
/// Maintenance accepts schema-one and schema-two TOML even when current startup validation would
/// reject its settings. Repair must preserve those bytes so the user can correct them afterwards.
/// # Errors
/// Rejects oversized/non-UTF-8 documents, malformed TOML and an unsupported version.
pub fn validate_preserved(bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAX_STARTUP_CONFIGURATION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved startup configuration exceeds its byte bound",
        ));
    }
    let value: toml::Value =
        std::str::from_utf8(bytes).map_err(io::Error::other)?.parse().map_err(io::Error::other)?;
    if !matches!(value.get("schema_version").and_then(toml::Value::as_integer), Some(1 | 2)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved startup configuration requires a compatible installer",
        ));
    }
    Ok(())
}
