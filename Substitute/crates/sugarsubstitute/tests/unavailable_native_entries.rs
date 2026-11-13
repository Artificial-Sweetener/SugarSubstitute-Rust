// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native entries without a host on this platform fail with their declared adapter, never silently.
#![cfg(not(windows))]

use std::{error::Error, process::Command};

/// Run one native entry and require a failure that names its unavailable adapter.
fn declared_unavailable(binary: &str, adapter: &str) -> Result<(), Box<dyn Error>> {
    let output = Command::new(binary).output()?;
    assert!(!output.status.success(), "{adapter} entry reported success");
    let diagnostics = String::from_utf8(output.stderr)?;
    assert!(
        diagnostics.contains("operation=\"native_adapter_unavailable\"")
            && diagnostics.contains(&format!("adapter=\"{adapter}\"")),
        "{adapter} entry did not declare its unavailable adapter: {diagnostics}"
    );
    Ok(())
}

#[test]
fn canvas_viewer_declares_its_unavailable_native_adapter() -> Result<(), Box<dyn Error>> {
    declared_unavailable(env!("CARGO_BIN_EXE_canvas-viewer"), "viewer")
}

#[test]
fn launcher_setup_and_updater_declare_their_unavailable_native_adapters()
-> Result<(), Box<dyn Error>> {
    declared_unavailable(env!("CARGO_BIN_EXE_sugarsubstitute-launcher"), "launcher")?;
    declared_unavailable(env!("CARGO_BIN_EXE_sugarsubstitute-setup"), "installer")?;
    declared_unavailable(env!("CARGO_BIN_EXE_sugarsubstitute-updater"), "updater")
}
