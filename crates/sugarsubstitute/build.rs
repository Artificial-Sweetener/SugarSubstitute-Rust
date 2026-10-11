// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Embed the application-owned Windows activation, privilege and DPI contract.

use std::{error::Error, path::PathBuf};

/// Apply the manifest to executable and test targets using the selected MSVC linker.
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
