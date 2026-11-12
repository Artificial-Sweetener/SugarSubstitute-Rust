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
        Self::populate(directory)
    }

    /// Create the fixture workspace as a named folder of a caller-owned monorepo directory.
    ///
    /// The workspace folder name starts with `name`; `parent` must outlive the fixture.
    pub(crate) fn new_in(parent: &Path, name: &str) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix(name)
            .tempdir_in(parent)
            .map_err(|error| Failure::external("create fixture", error))?;
        Self::populate(directory)
    }

    /// Write the governed host crate into `directory` and describe its ownership.
    fn populate(directory: tempfile::TempDir) -> Result<Self> {
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

/// Keep a governed fixture workspace inside an enclosing monorepo directory for one test.
pub(crate) struct Monorepo {
    directory: tempfile::TempDir,
    pub(crate) workspace: Repository,
}

impl Monorepo {
    /// Create a monorepo whose `Substitute*` folder is a governed workspace with every authored
    /// source root present, so sibling-component checks see the real workspace shape.
    pub(crate) fn new() -> Result<Self> {
        let directory =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        let workspace = Repository::new_in(directory.path(), "Substitute")?;
        for root in ["tools", "native", "installer"] {
            std::fs::create_dir_all(workspace.root().join(root))
                .map_err(|error| Failure::external("create fixture root", error))?;
        }
        Ok(Self { directory, workspace })
    }

    /// Return the governed workspace root.
    pub(crate) fn root(&self) -> &Path {
        self.workspace.root()
    }

    /// Return the enclosing monorepo directory that holds sibling components.
    pub(crate) fn monorepo(&self) -> &Path {
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

/// Link `link` to the existing directory `target` the way an ordinary user account can.
///
/// Windows uses a directory junction, which needs neither elevation nor developer mode; other
/// platforms use a symbolic link.
pub(crate) fn directory_link(target: &Path, link: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map_err(|error| Failure::external("create fixture junction", error))?;
        crate::error::require(output.status.success(), "create fixture junction")
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
            .map_err(|error| Failure::external("create fixture link", error))
    }
}
