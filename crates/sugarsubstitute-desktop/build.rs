// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native adapter tests under the same Windows activation contract as the host.

use std::{error::Error, path::PathBuf};

/// Embed the shared activation manifest in this adapter's native test executables.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=../../assets/windows/application.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows"
        && std::env::var("CARGO_CFG_TARGET_ENV")? == "msvc"
    {
        let manifest = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR")
                .ok_or("Cargo did not provide the package manifest directory")?,
        )
        .join("../../assets/windows/application.manifest")
        .canonicalize()?;
        println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo::rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    Ok(())
}
