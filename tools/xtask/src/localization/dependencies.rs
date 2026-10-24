// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resolve npm alias identities before allowing authored worker/renderer dependencies.
use crate::error::{Failure, Result, read, require};
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
/// An npm alias binds a different import name to its actual package identity.
fn identity<'a>(binding: &'a str, version: &'a str) -> &'a str {
    let Some(actual) = version.strip_prefix("npm:") else {
        return binding;
    };
    actual.rfind('@').filter(|index| *index > 0).map_or(actual, |index| &actual[..index])
}
/// Inspect all declared dependency classes without relying on JavaScript import spelling.
pub(super) fn check(path: &Path) -> Result<()> {
    let source: serde_json::Value = serde_json::from_str(&read(path)?)
        .map_err(|cause| Failure::external("parse script dependency manifest", cause))?;
    require(source.is_object(), "localization: script manifest must be an object")?;
    for key in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        let Some(values) = source.get(key) else {
            continue;
        };
        let values = values.as_object().ok_or_else(|| {
            Failure::Policy("localization: dependency inventory must be an object".into())
        })?;
        for (binding, version) in values {
            let version = version.as_str().ok_or_else(|| {
                Failure::Policy("localization: dependency identity must be explicit".into())
            })?;
            require(
                !independent(identity(binding, version)),
                format!(
                    "localization: independent script localizer {} via {binding}",
                    identity(binding, version)
                ),
            )?;
        }
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
