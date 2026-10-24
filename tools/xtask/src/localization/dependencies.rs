// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resolve npm alias identities before allowing authored worker/renderer dependencies.
use crate::error::{Result, read, require};
use std::path::Path;

/// Known independent localization engines must not introduce another corpus or registry.
fn independent(name: &str) -> bool {
    matches!(
        name,
        "i18next"
            | "react-i18next"
            | "vue-i18n"
            | "@lingui/core"
            | "@intlify/core-base"
            | "intl-messageformat"
            | "@formatjs/intl"
    )
}
/// Inspect all declared dependency classes without relying on JavaScript import spelling.
pub(super) fn check(path: &Path) -> Result<()> {
    for identity in crate::script_dependencies::identities(&read(path)?)? {
        require(
            !independent(&identity),
            format!("localization: independent script localizer {identity}"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npm_aliases_scoped_names_and_all_dependency_classes_reject_another_localizer() -> Result<()>
    {
        let repository = crate::fixtures::Repository::new()?;
        let path = "installer/package.json";
        for kind in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"]
        {
            for (name, version) in [
                ("i18next", "=25.0.0"),
                ("ordinary", "npm:i18next@25.0.0"),
                ("ordinary", "npm:i18next"),
                ("ordinary", "npm:@lingui/core@5.0.0"),
                ("ordinary", "npm:@lingui/core"),
            ] {
                let manifest = serde_json::json!({(kind):{(name):version}});
                crate::fixtures::write(repository.root(), path, &manifest.to_string())?;
                assert!(check(&repository.root().join(path)).is_err(), "accepted {kind} {version}");
            }
        }
        crate::fixtures::write(
            repository.root(),
            path,
            r#"{"dependencies":{"typescript":"=5.9.3"}}"#,
        )?;
        check(&repository.root().join(path))
    }
}
