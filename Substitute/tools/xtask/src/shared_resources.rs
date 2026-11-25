// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep every resource file of the monorepo in one place.
//!
//! An icon or image both components use lives once, in SugarCubes' shared icons
//! (`Sugar/integrations/contracts/ui/icons`), read by the native host and served to the page.
//! A second copy of the same bytes anywhere in either component is refused, so a shared resource
//! cannot drift into two versions. Generated output, vendored third-party files and test fixtures
//! are not resources and are not compared.

use std::{collections::BTreeMap, path::Path};

use sha2::{Digest, Sha256};

use crate::error::{Failure, Result};

/// The file kinds that are resources.
const EXTENSIONS: &[&str] = &["svg", "png", "ico", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Refuse byte-identical resource files tracked anywhere in the monorepo.
pub(crate) fn check(workspace: &Path) -> Result<()> {
    let monorepo = workspace
        .parent()
        .ok_or_else(|| Failure::Policy("the workspace has no enclosing monorepo".into()))?;
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(monorepo)
        .output()
        .map_err(|error| Failure::external("list the monorepo's files", error))?;
    if !output.status.success() {
        return Err(Failure::Policy("git could not list the monorepo's files".into()));
    }
    let listed = String::from_utf8_lossy(&output.stdout);
    let mut by_content: BTreeMap<[u8; 32], Vec<String>> = BTreeMap::new();
    for path in listed.split('\0').filter(|path| is_resource(path)) {
        // A tracked file deleted in the working tree is not a copy.
        let Ok(bytes) = std::fs::read(monorepo.join(path)) else { continue };
        by_content.entry(Sha256::digest(&bytes).into()).or_default().push(path.to_owned());
    }
    let copies: Vec<String> = by_content
        .into_values()
        .filter(|paths| paths.len() > 1)
        .map(|paths| paths.join(" = "))
        .collect();
    if copies.is_empty() {
        return Ok(());
    }
    Err(Failure::Policy(format!(
        "resource files are duplicated; keep one copy in Sugar/integrations/contracts/ui/icons \
         (or the one component that uses it) and reference it: {}",
        copies.join("; ")
    )))
}

/// Whether a tracked path is a resource file this check compares.
fn is_resource(path: &str) -> bool {
    let Some((_, extension)) = path.rsplit_once('.') else { return false };
    if !EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) {
        return false;
    }
    let generated = path.starts_with("Sugar/web/");
    let excluded = path.split('/').any(|part| {
        matches!(part, "third_party" | "node_modules" | "tests" | "test" | "fixtures" | "target")
    });
    !generated && !excluded
}

#[cfg(test)]
mod tests {
    use super::is_resource;

    #[test]
    fn resources_are_compared_and_generated_vendored_and_test_files_are_not() {
        for path in [
            "Sugar/integrations/contracts/ui/icons/app-icon.ico",
            "Sugar/frontend/static/orb/orb_base.png",
            "Substitute/crates/sugarsubstitute-ui/assets/fluent/Home.svg",
        ] {
            assert!(is_resource(path), "{path}");
        }
        for path in [
            "Sugar/web/icons/shared/app-icon-256.png",
            "Substitute/third_party/comfy-desktop/resources/installerIcon.ico",
            "Sugar/tests/surface/fixtures/card.png",
            "Sugar/frontend/static/orb/orb.ts",
            "README",
        ] {
            assert!(!is_resource(path), "{path}");
        }
    }
}
