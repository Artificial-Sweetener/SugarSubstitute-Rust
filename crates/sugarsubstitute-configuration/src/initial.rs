// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initial installed startup values; existing settings are preserved by the storage owner.
use crate::startup::{MAX_STARTUP_CONFIGURATION_BYTES, parse_startup_configuration};
use std::{io, path::Path};

/// Produce the implemented managed startup document from explicit installer selections.
/// Port 8188, profile `default`, dark appearance and full motion are initial values;
/// startup requires every persisted field explicitly and never reapplies these defaults.
/// # Errors
/// Rejects invalid paths, locale, output argument or a document above the startup byte bound.
pub fn managed(
    root: &Path,
    interpreter: &Path,
    outputs: &Path,
    locale: &str,
) -> io::Result<String> {
    let unicode = |path: &Path| {
        path.to_str().map(str::to_owned).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Installed startup path is not Unicode")
        })
    };
    let root = unicode(root)?;
    let interpreter = unicode(interpreter)?;
    let outputs = unicode(outputs)?;
    let document = toml::to_string(&serde_json::json!({
        "schema_version": 1, "mode": "launch", "port": 8188, "profile": "default",
        "locale": locale, "theme": "dark", "reduced_motion": false,
        "launch": { "root": root, "interpreter": interpreter,
            "arguments": ["--output-directory", outputs] },
    }))
    .map_err(io::Error::other)?;
    parse_startup_configuration(document.as_bytes()).map_err(io::Error::other)?;
    Ok(document)
}

/// Check the historical preservation contract without reserializing user-authored text.
/// Maintenance accepts schema-one TOML even when current startup validation would reject
/// its settings. Repair must preserve those bytes so the user can correct them afterwards.
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
    if value.get("schema_version").and_then(toml::Value::as_integer) != Some(1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved startup configuration requires a compatible installer",
        ));
    }
    Ok(())
}
