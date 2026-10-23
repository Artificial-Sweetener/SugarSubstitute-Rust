// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emit native launch compatibility and embed Windows activation, privilege and DPI policy.

use std::{error::Error, io::Read, path::PathBuf};

/// Emit the producing build's contract and apply its native manifest with the selected linker.
fn main() -> Result<(), Box<dyn Error>> {
    emit_launch_contracts()?;
    for (variable, name, limit) in [
        ("SUGARSUBSTITUTE_SETUP_DISTRIBUTION", "installer-distribution.json", 4 * 1024 * 1024),
        ("SUGARSUBSTITUTE_UPDATE_DISTRIBUTION", "update-distribution.json", 16 * 1024),
    ] {
        embed_distribution(variable, name, limit)?;
    }
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

/// Bind each distribution input at build time and overwrite stale outputs when it is disabled.
fn embed_distribution(variable: &str, name: &str, limit: u64) -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-env-changed={variable}");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo output directory absent")?)
        .join(name);
    let mut bytes = Vec::new();
    if let Some(source) = std::env::var_os(variable) {
        let path = PathBuf::from(source);
        let text = path.to_str().ok_or("Distribution path must be UTF-8")?;
        if !path.is_absolute() || text.contains(['\r', '\n']) {
            return Err("Distribution requires an absolute build-input path".into());
        }
        println!("cargo::rerun-if-changed={text}");
        let file = std::fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err("Distribution must be a regular file".into());
        }
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() as u64 > limit {
            return Err("Distribution is empty or exceeds its encoded bound".into());
        }
    }
    std::fs::write(output, bytes)?;
    Ok(())
}

/// Emit the same declaration compiled into this application for Cargo artifact assembly.
/// A build without the Windows native entry adapter must not advertise that protocol.
fn emit_launch_contracts() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo output directory absent")?);
    let native = std::env::var("CARGO_CFG_TARGET_OS")? == "windows"
        && std::env::var_os("CARGO_FEATURE_WINDOWS_NATIVE_HOST").is_some();
    for (source, name, bytes) in [
        (
            "current.json",
            "sugarsubstitute-application.json",
            include_bytes!("../sugarsubstitute-contracts/src/application_launch/current.json")
                .as_slice(),
        ),
        (
            "launcher.json",
            "sugarsubstitute-launcher.json",
            include_bytes!("../sugarsubstitute-contracts/src/application_launch/launcher.json")
                .as_slice(),
        ),
        (
            "updater.json",
            "sugarsubstitute-updater.json",
            include_bytes!("../sugarsubstitute-contracts/src/application_launch/updater.json")
                .as_slice(),
        ),
    ] {
        println!(
            "cargo::rerun-if-changed=../sugarsubstitute-contracts/src/application_launch/{source}"
        );
        let target = output.join(name);
        if native {
            std::fs::write(target, bytes)?;
        } else {
            match std::fs::remove_file(target) {
                Ok(()) => {}
                Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause.into()),
            }
        }
    }
    Ok(())
}
