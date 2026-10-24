// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check direct npm network identities, including npm aliases, against exact reviewed manifests.
use super::*;
/// A locked script dependency boundary; external Electron networking remains upstream-owned.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    path: String,
    lockfile: String,
    owner: String,
    source_sha256: String,
    network_dependencies: Vec<String>,
    rationale: String,
}
/// Canonical direct network client identities are independent of the import binding's spelling.
pub(super) const CLIENTS: &[&str] = &[
    "electron",
    "@electron/get",
    "axios",
    "got",
    "node-fetch",
    "undici",
    "cross-fetch",
    "superagent",
    "request",
    "ws",
    "socket.io",
    "socket.io-client",
    "needle",
    "ky",
];
/// Recognize reviewed HTTP/socket client package identities without considering local aliases.
pub(super) fn client(name: &str) -> bool {
    CLIENTS.contains(&name)
}
/// Inspect all direct dependency kinds and normalize npm package aliases before review comparison.
fn identities(source: &str) -> Result<BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_str(source)
        .map_err(|e| Failure::external("parse script dependency manifest", e))?;
    require(value.is_object(), "networking: script manifest must be an object")?;
    let mut found = BTreeSet::new();
    for section in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        if let Some(dependencies) = value.get(section) {
            let dependencies = dependencies.as_object().ok_or_else(|| {
                Failure::Policy("networking: dependency section must be an object".into())
            })?;
            for (name, version) in dependencies {
                let version = version.as_str().ok_or_else(|| {
                    Failure::Policy("networking: dependency version must be a string".into())
                })?;
                let identity = version
                    .strip_prefix("npm:")
                    .map(|alias| {
                        alias
                            .rfind('@')
                            .filter(|index| *index > 0)
                            .map_or(alias, |index| &alias[..index])
                    })
                    .unwrap_or(name);
                if client(identity) {
                    found.insert(identity.to_owned());
                }
            }
        }
    }
    Ok(found)
}
/// Require every discovered authored package.json and reject stale or foreign network allowances.
pub(super) fn check(
    root: &Path,
    architecture: &Architecture,
    records: &[Manifest],
    paths: &BTreeSet<String>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for record in records {
        require(
            seen.insert(record.path.clone()) && paths.contains(&record.path),
            "networking: duplicate or stale script manifest",
        )?;
        require(
            architecture.owners.iter().any(|owner| owner.package == record.owner)
                && !record.rationale.trim().is_empty(),
            "networking: unknown or unexplained script boundary owner",
        )?;
        resolve(root, &record.path)?;
        resolve(root, &record.lockfile)?;
        require(
            fingerprint(root, &[record.path.clone(), record.lockfile.clone()])?
                == record.source_sha256,
            "networking: stale script dependency review",
        )?;
        let declared = record.network_dependencies.iter().cloned().collect::<BTreeSet<_>>();
        require(
            declared.len() == record.network_dependencies.len()
                && declared == identities(&read(&root.join(&record.path))?)?,
            "networking: unregistered or stale script network dependency identity",
        )?;
        require(
            record.path == "installer/package.json"
                && record.owner == "sugarsubstitute-xtask"
                && declared == BTreeSet::from(["electron".into()]),
            "networking: script network dependency lacks the registered upstream Electron boundary",
        )?;
    }
    require(seen == *paths, "networking: missing required script dependency coverage")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npm_aliases_optional_dev_and_peer_dependencies_preserve_client_identity() -> Result<()> {
        for section in
            ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"]
        {
            for version in
                ["npm:axios@1.2.3", "npm:axios", "npm:@electron/get@3.0.0", "npm:@electron/get"]
            {
                let source = serde_json::json!({section:{"harmless":version}}).to_string();
                assert_eq!(identities(&source)?.len(), 1);
            }
        }
        assert!(identities(r#"{"dependencies":{"electron":"40.4.1"}}"#)?.contains("electron"));
        assert!(identities(r#"{"dependencies":{"url":"2.0.0"}}"#)?.is_empty());
        assert!(identities(r#"{"dependencies":[]}"#).is_err());
        Ok(())
    }
    #[test]
    fn unreviewed_manifests_and_alternate_script_network_clients_reject() -> Result<()> {
        for variant in 0..8 {
            let mut repository = crate::fixtures::Repository::new()?;
            repository.architecture.owners[0].package = "sugarsubstitute-xtask".into();
            crate::fixtures::write(
                repository.root(),
                "installer/package.json",
                r#"{"dependencies":{"electron":"40.4.1"}}"#,
            )?;
            crate::fixtures::write(repository.root(), "installer/package-lock.json", "{}")?;
            let paths = BTreeSet::from(["installer/package.json".into()]);
            let mut record = Manifest {
                path: "installer/package.json".into(),
                lockfile: "installer/package-lock.json".into(),
                owner: "sugarsubstitute-xtask".into(),
                source_sha256: fingerprint(
                    repository.root(),
                    &["installer/package.json".into(), "installer/package-lock.json".into()],
                )?,
                network_dependencies: vec!["electron".into()],
                rationale: "Actual upstream worker networking".into(),
            };
            check(
                repository.root(),
                &repository.architecture,
                std::slice::from_ref(&record),
                &paths,
            )?;
            match variant {
                0 => {
                    assert!(
                        check(repository.root(), &repository.architecture, &[], &paths).is_err()
                    );
                    continue;
                }
                1 => record.owner = "foreign".into(),
                2 => record.source_sha256 = "stale".into(),
                3 => record.network_dependencies.clear(),
                4 => record.network_dependencies.push("electron".into()),
                5 => record.rationale.clear(),
                6 => crate::fixtures::write(
                    repository.root(),
                    "installer/package.json",
                    r#"{"dependencies":{"innocent":"npm:axios@1.0.0"}}"#,
                )?,
                _ => record.lockfile = "../foreign".into(),
            }
            assert!(check(repository.root(), &repository.architecture, &[record], &paths).is_err());
        }
        Ok(())
    }
}
