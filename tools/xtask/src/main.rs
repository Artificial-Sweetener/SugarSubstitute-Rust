// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Provide the same portable verification entry point for local work, hooks and CI.

mod architecture;
mod catalog;
mod command;
mod error;
mod execution;
mod execution_source;
#[cfg(test)]
mod execution_tests;
#[cfg(test)]
mod fixtures;
mod governance;
mod metrics;
mod policy;
mod source;
mod structure;
mod testing;
mod vendor;
mod workspace;

use std::{path::Path, process::ExitCode};

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
            "cargo xtask governance | check | verify | help\ngovernance: repository policies\ncheck: policies, formatting and strict Clippy\nverify: check plus tests and warning-free Rustdoc"
        );
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Failure::Policy("xtask must live under tools/xtask".into()))?;
    governance::check(root)?;
    if task == "governance" {
        return Ok(());
    }
    command::cargo(root, &["fmt", "--all", "--", "--check"], false)?;
    command::cargo(
        root,
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        false,
    )?;
    if task == "verify" {
        command::cargo(root, &["test", "--locked", "--workspace", "--all-features"], false)?;
        command::cargo(
            root,
            &["doc", "--locked", "--workspace", "--all-features", "--no-deps"],
            true,
        )?;
    }
    Ok(())
}

/// Require one supported task so accidental filters cannot bypass verification.
fn select_task(arguments: &[String]) -> Result<&str> {
    match arguments {
        [] => Ok("help"),
        [task] if matches!(task.as_str(), "help" | "--help" | "-h") => Ok("help"),
        [task] if matches!(task.as_str(), "governance" | "check" | "verify") => Ok(task),
        _ => Err(Failure::Policy(
            "use cargo xtask governance, check, verify or help; extra arguments are rejected"
                .into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject task typos and extra arguments that might narrow the required gate.
    #[test]
    fn dispatch_rejects_unknown_or_extra_arguments() -> Result<()> {
        assert_eq!(select_task(&["verify".into()])?, "verify");
        assert_eq!(select_task(&[])?, "help");
        assert!(select_task(&["verfy".into()]).is_err());
        assert!(select_task(&["verify".into(), "--skip-tests".into()]).is_err());
        Ok(())
    }
}
