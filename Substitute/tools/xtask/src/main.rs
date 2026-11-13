// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Provide the same portable verification entry point for local work, hooks and CI.

mod architecture;
#[cfg(test)]
mod architecture_source_tests;
mod authored_sources;
mod cache_storage;
mod capability_source;
mod catalog;
mod command;
mod configuration;
mod diagnostics_gate;
mod error;
mod execution;
mod execution_exports;
mod execution_exports_declarations;
mod execution_exports_macros;
mod execution_exports_modules;
mod execution_exports_names;
#[cfg(test)]
mod execution_exports_resolution_tests;
#[cfg(test)]
mod execution_exports_tests;
mod execution_source;
#[cfg(test)]
mod execution_tests;
#[cfg(test)]
mod fixtures;
mod gates;
mod governance;
mod infrastructure;
mod localization;
mod metrics;
mod networking;
mod policy;
mod script_dependencies;
mod shared_components;
mod source;
mod source_tokens;
mod structure;
mod testing;
mod ui;
mod ui_source;
mod vendor;
mod workspace;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use error::{Failure, Result};

/// Report developer-facing errors and preserve a nonzero status for hooks and CI.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("SugarSubstitute verification: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch explicit tasks without accepting ignored or misspelled arguments.
fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let task = select_task(&arguments)?;
    if task == "help" {
        eprintln!(
            "cargo xtask governance | ui | check | verify | storage-inventory | help\ngovernance: repository policies\nui: native UI ownership policy only\ncheck: policies, formatting and strict Clippy\nverify: check plus tests and warning-free Rustdoc"
        );
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Failure::Policy("xtask must live under tools/xtask".into()))?;
    let invocation = std::env::current_dir()
        .map_err(|error| Failure::external("read verification working directory", error))?;
    let root = invocation_root(root, &invocation)?;
    let root = root.as_path();
    if task == "storage-inventory" {
        let architecture = policy::load(root, "governance/architecture/policy.toml")?;
        let workspace = workspace::Workspace::load(root)?;
        let roots = ["crates", "tools", "native", "installer"].map(String::from);
        let inventory = infrastructure::inventory(root, &architecture, &workspace, &roots)?;
        let json = serde_json::to_string_pretty(&inventory)
            .map_err(|cause| Failure::external("encode storage inventory", cause))?;
        println!("{json}");
        return Ok(());
    }
    if task == "ui" {
        let architecture = policy::load(root, "governance/architecture/policy.toml")?;
        let workspace = workspace::Workspace::load(root)?;
        let ui = policy::load(root, "governance/ui/policy.toml")?;
        return ui::check(root, &ui, &architecture, &workspace);
    }
    governance::check(root)?;
    if task == "governance" {
        return Ok(());
    }
    command::cargo(root, &["fmt", "--all", "--", "--check"], false)?;
    for leg in gates::CLIPPY {
        command::cargo(root, leg, false)?;
    }
    if task == "verify" {
        for leg in gates::TESTS {
            command::cargo(root, leg, false)?;
        }
        command::cargo(root, gates::DOCUMENTATION, true)?;
    }
    Ok(())
}

/// Reject a cached verification executable from another checkout before running any gate.
fn invocation_root(compiled: &Path, invoked: &Path) -> Result<PathBuf> {
    let compiled = compiled
        .canonicalize()
        .map_err(|error| Failure::external("resolve compiled verification checkout", error))?;
    let invoked = invoked
        .canonicalize()
        .map_err(|error| Failure::external("resolve invoked verification checkout", error))?;
    if compiled != invoked {
        return Err(Failure::Policy(
            "run verification from the repository root with a target directory owned by this checkout; rebuild sugarsubstitute-xtask if its cached executable belongs to another checkout".into(),
        ));
    }
    Ok(invoked)
}

/// Require one supported task so accidental filters cannot bypass verification.
fn select_task(arguments: &[String]) -> Result<&str> {
    match arguments {
        [] => Ok("help"),
        [task] if matches!(task.as_str(), "help" | "--help" | "-h") => Ok("help"),
        [task] if matches!(task.as_str(), "governance" | "ui" | "check" | "verify" | "storage-inventory") => Ok(task),
        _ => Err(Failure::Policy(
            "use cargo xtask governance, ui, check, verify, storage-inventory or help; extra arguments are rejected"
                .into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shared Cargo artifact must never silently verify a different checkout.
    #[test]
    fn verification_rejects_other_checkout_and_accepts_resolved_root() -> Result<()> {
        let compiled = crate::fixtures::Repository::new()?;
        let invoked = crate::fixtures::Repository::new()?;
        assert!(invocation_root(compiled.root(), invoked.root()).is_err());
        assert!(invocation_root(compiled.root(), &compiled.root().join("missing")).is_err());
        assert_eq!(
            invocation_root(compiled.root(), &compiled.root().join("."))?,
            compiled
                .root()
                .canonicalize()
                .map_err(|error| Failure::external("resolve fixture", error))?,
        );
        Ok(())
    }

    /// Reject task typos and extra arguments that might narrow the required gate.
    #[test]
    fn dispatch_rejects_unknown_or_extra_arguments() -> Result<()> {
        assert_eq!(select_task(&["verify".into()])?, "verify");
        assert_eq!(select_task(&[])?, "help");
        assert_eq!(select_task(&["ui".into()])?, "ui");
        assert!(select_task(&["ui".into(), "--skip-modal".into()]).is_err());
        assert!(select_task(&["verfy".into()]).is_err());
        assert!(select_task(&["verify".into(), "--skip-tests".into()]).is_err());
        Ok(())
    }
}
