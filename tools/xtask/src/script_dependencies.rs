// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve direct npm dependency identities consistently across infrastructure gates.
use crate::error::{Failure, Result, require};
use std::collections::BTreeSet;

/// Inspect every direct dependency class, resolving scoped and unscoped npm aliases.
/// Import bindings do not change a package's authority or infrastructure classification.
pub(crate) fn identities(source: &str) -> Result<BTreeSet<String>> {
    let manifest: serde_json::Value = serde_json::from_str(source)
        .map_err(|cause| Failure::external("parse script dependency manifest", cause))?;
    require(manifest.is_object(), "script dependencies: manifest must be an object")?;
    let mut identities = BTreeSet::new();
    for kind in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        let Some(values) = manifest.get(kind) else { continue };
        let values = values.as_object().ok_or_else(|| {
            Failure::Policy("script dependencies: dependency class must be an object".into())
        })?;
        for (binding, version) in values {
            let version = version.as_str().ok_or_else(|| {
                Failure::Policy("script dependencies: package identity must be explicit".into())
            })?;
            let identity = match version.strip_prefix("npm:") {
                Some(actual) => actual
                    .rfind('@')
                    .filter(|index| *index > 0)
                    .map_or(actual, |index| &actual[..index]),
                None => binding,
            };
            require(!identity.is_empty(), "script dependencies: empty package identity")?;
            identities.insert(identity.to_owned());
        }
    }
    Ok(identities)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_keep_scoped_and_unscoped_identities_in_every_dependency_class() -> Result<()> {
        for kind in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"]
        {
            for (version, expected) in [
                ("npm:axios@1.0", "axios"),
                ("npm:axios", "axios"),
                ("npm:@lingui/core@5.0", "@lingui/core"),
                ("npm:@lingui/core", "@lingui/core"),
            ] {
                let source = serde_json::json!({kind:{"ordinary":version}}).to_string();
                assert_eq!(identities(&source)?, BTreeSet::from([expected.to_owned()]));
            }
        }
        assert_eq!(
            identities(r#"{"dependencies":{"typescript":"=5.9.3"}}"#)?,
            BTreeSet::from(["typescript".to_owned()])
        );
        Ok(())
    }
    #[test]
    fn ambiguous_manifest_and_dependency_shapes_reject() {
        for source in [
            "null",
            "[]",
            "true",
            "not-json",
            r#"{"dependencies":[]}"#,
            r#"{"devDependencies":{"ordinary":null}}"#,
            r#"{"optionalDependencies":{"ordinary":{}}}"#,
            r#"{"peerDependencies":{"ordinary":"npm:"}}"#,
        ] {
            assert!(identities(source).is_err(), "accepted {source}");
        }
    }
}
