// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Build isolated synthetic repositories for governance rejection proof.

use std::path::Path;

use crate::{
    error::{Failure, Result},
    policy::{Architecture, Owner},
    workspace::Workspace,
};

/// Keep fixture files and declared ownership alive for one deterministic test.
pub(crate) struct Repository {
    directory: tempfile::TempDir,
    pub(crate) architecture: Architecture,
    pub(crate) workspace: Workspace,
}

impl Repository {
    /// Create a minimally governed host without invoking a compiler or network.
    pub(crate) fn new() -> Result<Self> {
        let directory =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        let root = directory.path();
        write(
            root,
            "crates/host/Cargo.toml",
            "[package]\nname = 'host'\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nauthors.workspace = true\npublish.workspace = true\n[lints]\nworkspace = true\n",
        )?;
        write(
            root,
            "crates/host/src/lib.rs",
            "// SPDX-License-Identifier: GPL-3.0-or-later\n// Copyright (C) 2026 Artificial Sweetener and contributors\n//! Fixture host.\n#[cfg(test)] mod tests { #[test] fn behavior() {} }\n",
        )?;
        let workspace: Workspace = serde_json::from_value(serde_json::json!({
            "workspace_members": ["fixture-host"],
            "packages": [{"name": "host", "id": "fixture-host",
                "manifest_path": root.join("crates/host/Cargo.toml"), "dependencies": []}]
        }))
        .map_err(|error| Failure::external("construct fixture workspace", error))?;
        let architecture = Architecture {
            schema_version: 1,
            soft_lines: 350,
            hard_lines: 500,
            source_roots: vec!["crates".into()],
            layers: vec!["domain".into(), "host".into(), "tooling".into()],
            owners: vec![Owner {
                package: "host".into(),
                root: "crates/host".into(),
                auxiliary_roots: Vec::new(),
                auxiliary_files: Vec::new(),
                layer: "host".into(),
                concern: "fixture host".into(),
            }],
        };
        Ok(Self { directory, architecture, workspace })
    }

    /// Return the fixture's repository root for the duration of its owner.
    pub(crate) fn root(&self) -> &Path {
        self.directory.path()
    }
}

/// Write only inside an owned fixture with explicit error context.
pub(crate) fn write(root: &Path, relative: &str, contents: &str) -> Result<()> {
    let path = root.join(relative);
    let parent =
        path.parent().ok_or_else(|| Failure::Policy("fixture path needs a parent".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| Failure::external("create fixture directories", error))?;
    std::fs::write(path, contents)
        .map_err(|error| Failure::external(format!("write fixture {relative}"), error))
}
