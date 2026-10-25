// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain ordinary OS paths while removing inherited interpreter/runtime injection.
use std::{error::Error, ffi::OsString, os::windows::ffi::OsStrExt};
use sugarsubstitute_supervision::LaunchEnvironment;

/// Build the bounded worker environment without modifying the current process.
/// Interpreter injection variables are removed; native preparation also uses this snapshot.
/// Rejects oversized or invalid UTF-16 environment blocks.
pub(crate) fn capture() -> Result<LaunchEnvironment, Box<dyn Error + Send + Sync>> {
    prepare(std::env::vars_os())
}

/// Keep runtime diagnostics and disposable cache writes on the host's explicit owners.
fn prepare(
    entries: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<LaunchEnvironment, Box<dyn Error + Send + Sync>> {
    let mut snapshot = Vec::new();
    for (name, value) in entries {
        let upper = name.to_string_lossy().to_ascii_uppercase();
        if matches!(
            upper.as_str(),
            "ELECTRON_RUN_AS_NODE"
                | "NODE_OPTIONS"
                | "NODE_PATH"
                | "NODE_TLS_REJECT_UNAUTHORIZED"
                | "NODE_COMPILE_CACHE"
                | "NODE_V8_COVERAGE"
                | "NODE_REDIRECT_WARNINGS"
                | "PYTHONHOME"
                | "PYTHONPATH"
                | "VIRTUAL_ENV"
                | "COMFY_STANDALONE_BASE_URL"
        ) {
            continue;
        }
        snapshot.extend(name.encode_wide());
        snapshot.push(u16::from(b'='));
        snapshot.extend(value.encode_wide());
        snapshot.push(0);
        if snapshot.len() > 32766 {
            return Err("Worker environment exceeds its bound".into());
        }
    }
    snapshot.push(0);
    if snapshot.len() == 1 {
        snapshot.push(0);
    }
    Ok(LaunchEnvironment::from_snapshot_utf16(&snapshot, &[])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_worker_preserves_proxy_unicode_and_paths_without_runtime_side_effect_settings()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let entries = [
            ("Path", r"E:\用户\tools"),
            ("HTTP_PROXY", "http://proxy.invalid:8080"),
            ("NO_PROXY", "localhost"),
            ("node_compile_cache", r"E:\outside-cache"),
            ("NODE_V8_COVERAGE", r"E:\outside-coverage"),
            ("Node_Redirect_Warnings", r"E:\outside-log"),
            ("NODE_OPTIONS", "--require untrusted"),
            ("NODE_PATH", r"E:\outside-modules"),
            ("NODE_TLS_REJECT_UNAUTHORIZED", "0"),
        ];
        let environment = prepare(entries.map(|(name, value)| (name.into(), value.into())))?;
        let rows = environment
            .as_utf16()
            .split(|unit| *unit == 0)
            .filter(|row| !row.is_empty())
            .map(String::from_utf16)
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        assert_eq!(
            rows,
            [
                r"Path=E:\用户\tools".to_owned(),
                "HTTP_PROXY=http://proxy.invalid:8080".to_owned(),
                "NO_PROXY=localhost".to_owned(),
            ]
            .into_iter()
            .collect()
        );
        Ok(())
    }

    #[test]
    fn oversized_worker_environment_is_rejected_before_native_launch() {
        assert!(prepare([(OsString::from("PATH"), OsString::from("x".repeat(32767)))]).is_err());
    }
}
