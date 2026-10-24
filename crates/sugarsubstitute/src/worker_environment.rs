// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain ordinary OS paths while removing inherited interpreter/runtime injection.
use std::{error::Error, os::windows::ffi::OsStrExt};
use sugarsubstitute_supervision::LaunchEnvironment;

/// Build the bounded worker environment without modifying the current process.
/// Interpreter injection variables are removed; native preparation also uses this snapshot.
/// Rejects oversized or invalid UTF-16 environment blocks.
pub(crate) fn capture() -> Result<LaunchEnvironment, Box<dyn Error + Send + Sync>> {
    let mut snapshot = Vec::new();
    for (name, value) in std::env::vars_os() {
        let upper = name.to_string_lossy().to_ascii_uppercase();
        if matches!(
            upper.as_str(),
            "ELECTRON_RUN_AS_NODE"
                | "NODE_OPTIONS"
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
