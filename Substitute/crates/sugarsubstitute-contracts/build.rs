// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile the shared diagnostic source contract into immutable runtime data.
#[path = "src/diagnostics/schema.rs"]
mod schema;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/diagnostics/current.json");
    println!("cargo:rerun-if-changed=src/diagnostics/schema.rs");
    let schema = schema::Schema::parse(include_str!("src/diagnostics/current.json"))?;
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Missing build output")?);
    std::fs::write(out.join("diagnostics_contract.rs"), schema.rust()?)?;
    Ok(())
}
