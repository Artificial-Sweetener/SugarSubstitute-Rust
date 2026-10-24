// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose repository checks while keeping each concern's policy owner independent.

use std::path::Path;

use crate::{
    architecture, cache_storage, catalog, configuration,
    error::{Failure, Result, read, require},
    execution, infrastructure,
    policy::{self, Architecture, Debt, Records, Testing, Waiver},
    structure, testing, vendor,
    workspace::Workspace,
};

/// Validate the actual source tree, crate graph and all supported registries.
pub(crate) fn check(root: &Path) -> Result<()> {
    check_toolchain(root)?;
    let architecture: Architecture = policy::load(root, "governance/architecture/policy.toml")?;
    let workspace = Workspace::load(root)?;
    architecture::check(root, &architecture, &workspace)?;
    let configuration: configuration::Policy =
        policy::load(root, "governance/infrastructure/configuration.toml")?;
    configuration::check(root, &architecture, &workspace, &configuration)?;
    let execution: execution::Policy = policy::load(root, "governance/execution/policy.toml")?;
    execution::check(root, &execution, &architecture, &workspace)?;
    let infrastructure: infrastructure::Policy =
        policy::load(root, "governance/infrastructure/storage.toml")?;
    infrastructure::check(
        root,
        &architecture,
        &infrastructure,
        time::OffsetDateTime::now_utc().date(),
    )?;
    let debt: Records<Debt> = policy::load(root, "governance/architecture/debt.toml")?;
    let waivers: Records<Waiver> = policy::load(root, "governance/architecture/waivers.toml")?;
    structure::check(root, &architecture, &debt, &waivers, time::OffsetDateTime::now_utc().date())?;
    let testing: Testing = policy::load(root, "governance/testing/policy.toml")?;
    testing::check(root, &testing, &architecture)?;
    let persistence: catalog::Persistence =
        policy::load(root, "governance/persistence/catalog.toml")?;
    catalog::check_persistence(root, &architecture, &persistence)?;
    let caches: catalog::Caches = policy::load(root, "governance/cache/catalog.toml")?;
    catalog::check_caches(root, &architecture, &caches)?;
    let storage: cache_storage::Storage = policy::load(root, "governance/cache/storage.toml")?;
    cache_storage::check(root, &architecture, &caches, &storage)?;
    let vendors: vendor::Manifest = policy::load(root, "third_party/manifest.toml")?;
    vendor::check(root, &vendors)?;
    eprintln!("SugarSubstitute governance passed");
    Ok(())
}

/// Require one concrete Rust version and prevent ambient overrides from hiding drift.
fn check_toolchain(root: &Path) -> Result<()> {
    let toolchain: toml::Value = toml::from_str(&read(&root.join("rust-toolchain.toml"))?)
        .map_err(|error| Failure::external("parse Rust toolchain", error))?;
    let manifest: toml::Value = toml::from_str(&read(&root.join("Cargo.toml"))?)
        .map_err(|error| Failure::external("parse workspace manifest", error))?;
    let version = toolchain
        .get("toolchain")
        .and_then(|table| table.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| Failure::Policy("pin a concrete Rust channel".into()))?;
    require(
        version.split('.').count() == 3
            && version.split('.').all(|part| part.parse::<u32>().is_ok()),
        "Rust channel must pin a concrete three-part stable version",
    )?;
    require(
        manifest
            .get("workspace")
            .and_then(|table| table.get("package"))
            .and_then(|table| table.get("rust-version"))
            .and_then(toml::Value::as_str)
            == Some(version),
        "workspace rust-version must match rust-toolchain.toml",
    )?;
    let output = std::process::Command::new("rustc")
        .arg("--version")
        .current_dir(root)
        .output()
        .map_err(|error| Failure::external("inspect active Rust", error))?;
    require(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).split_whitespace().nth(1) == Some(version),
        format!("active Rust does not match {version}; remove ambient overrides"),
    )
}
