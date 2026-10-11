// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Check exact vendored-file provenance without treating Cargo dependencies as copies.

use std::{collections::BTreeSet, path::Path};

use serde::Deserialize;

use crate::{
    error::{Result, require},
    source::resolve,
};

/// Inventory third-party files copied into the application source tree.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) schema_version: u32,
    pub(crate) components: Vec<Component>,
}

/// Bind one vendored component to its license and exact consumed files.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Component {
    id: String,
    source_url: String,
    license: String,
    license_file: String,
    files: Vec<String>,
}

/// Reject missing provenance, license text, files and duplicate file ownership.
pub(crate) fn check(root: &Path, manifest: &Manifest) -> Result<()> {
    require(manifest.schema_version == 1, "third-party manifest requires schema one")?;
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    for component in &manifest.components {
        require(
            !component.id.trim().is_empty()
                && ids.insert(&component.id)
                && component.source_url.starts_with("https://")
                && !component.license.trim().is_empty()
                && !component.files.is_empty(),
            "vendored components require unique ids, source URL, license and exact files",
        )?;
        require(
            component.license_file.starts_with("third_party/licenses/")
                && resolve(root, &component.license_file)?.is_file(),
            format!("{}: store license text under third_party/licenses", component.id),
        )?;
        for file in &component.files {
            require(
                files.insert(file) && resolve(root, file)?.is_file(),
                format!("duplicate or missing vendored file: {file}"),
            )?;
        }
    }
    if !manifest.components.is_empty() {
        require(
            resolve(root, "third_party/NOTICE.md")?.is_file(),
            "vendored components require third_party/NOTICE.md",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject missing license/notice material and duplicate runtime file ownership.
    #[test]
    fn vendoring_requires_complete_unique_provenance() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        crate::fixtures::write(fixture.root(), "assets/vendor.txt", "asset")?;
        let component = || Component {
            id: "component".into(),
            source_url: "https://example.test/source".into(),
            license: "MIT".into(),
            license_file: "third_party/licenses/component.txt".into(),
            files: vec!["assets/vendor.txt".into()],
        };
        let mut manifest = Manifest { schema_version: 1, components: vec![component()] };
        assert!(check(fixture.root(), &manifest).is_err());
        crate::fixtures::write(
            fixture.root(),
            "third_party/licenses/component.txt",
            "fixture license",
        )?;
        assert!(check(fixture.root(), &manifest).is_err());
        crate::fixtures::write(fixture.root(), "third_party/NOTICE.md", "fixture notice")?;
        check(fixture.root(), &manifest)?;
        let mut duplicate = component();
        duplicate.id = "other".into();
        manifest.components.push(duplicate);
        assert!(check(fixture.root(), &manifest).is_err());
        Ok(())
    }
}
