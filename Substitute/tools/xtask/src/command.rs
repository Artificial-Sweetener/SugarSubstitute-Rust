// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Run repository-pinned Cargo tasks and preserve their failure status.

use std::{path::Path, process::Command};

use crate::error::{Failure, Result};

/// Invoke Cargo through argument lists, inheriting the selected Rust toolchain.
pub(crate) fn cargo(root: &Path, arguments: &[&str], documentation: bool) -> Result<()> {
    let executable = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(executable);
    command.current_dir(root).args(arguments);
    if documentation {
        let existing = std::env::var("RUSTDOCFLAGS").unwrap_or_default();
        command.env("RUSTDOCFLAGS", format!("{existing} -D warnings"));
    }
    eprintln!("cargo {}", arguments.join(" "));
    let status = command.status().map_err(|error| Failure::external("launch cargo", error))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::Process {
            command: format!("cargo {}", arguments.join(" ")),
            code: status.code(),
        })
    }
}
