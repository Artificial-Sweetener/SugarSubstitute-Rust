// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adversarial configuration access and strict review integrity fixtures.
use super::*;
use crate::fixtures::{self, Repository};

fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "adapter".into();
    repository.architecture.owners[0].auxiliary_roots.push("installer/worker".into());
    fixtures::write(repository.root(), "installer/worker/main.ts", "export {}")?;
    for directory in ["native", "tools"] {
        fixtures::write(repository.root(), &format!("{directory}/.keep"), "")?;
    }
    Ok((repository, Policy { schema_version: 1, owner: "host".into(), records: vec![] }))
}
fn check(repository: &Repository, policy: &Policy) -> Result<()> {
    super::check(repository.root(), &repository.architecture, &repository.workspace, policy)
}
fn register(repository: &Repository, policy: &mut Policy, path: &str, source: &str) -> Result<()> {
    fixtures::write(repository.root(), path, source)?;
    policy.records.push(Record {
        path: path.into(),
        owner: "host".into(),
        source_sha256: fingerprint(repository.root(), &[path.into()])?,
        capabilities: access::findings(source, path.ends_with(".rs"))?,
        rationale: "Known configuration boundary".into(),
    });
    Ok(())
}

#[test]
fn qualified_aliases_reexports_wrappers_and_macros_cannot_hide_environment() -> Result<()> {
    for source in [
        "fn f() { std::env::var(\"X\"); }",
        "use std::env::var_os as value; fn wrapper() { value(\"X\"); }",
        "pub use std::env::{vars_os as values};",
        "use std::env::{self as environment}; fn f() { environment::args_os(); }",
        "use std::*; fn f() { env::var(\"X\"); }",
        "macro_rules! bypass { () => { std::env::var(\"X\") } }",
        "macro_rules! bypass { ($a:ident,$b:ident,$c:ident) => { $a::$b::$c(\"X\") } } fn f(){ bypass!(std,env,var); }",
        "#[cfg(unix)] fn other_platform() { std::env::args(); }",
        "#[cfg(feature=\"optional\")] fn optional() { std::env::var_os(\"X\"); }",
        "#[cfg(test)] mod tests { fn f() { std::env::vars(); } }",
        "unsafe extern \"system\" { #[link_name=\"GetEnvironmentVariableW\"] fn hidden(); }",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/bin/auxiliary.RS", source)?;
        assert!(check(&repository, &policy).is_err(), "accepted {source}");
    }
    Ok(())
}

#[test]
fn low_level_document_and_persistent_backend_access_cannot_compete() -> Result<()> {
    for source in [
        "use toml as schema; fn f() { schema::from_str(input); }",
        "pub use toml::Value as Raw;",
        "use confy::load as read_settings;",
        "use windows::Win32::System::Registry::RegSetValueExW as save;",
        "fn write() { std::fs::write(\"startup.local.toml\", data); }",
        "fn write() { std::fs::File::create(\"sugarsubstitute-launch.local.json\"); }",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/bypass.rs", source)?;
        assert!(check(&repository, &policy).is_err(), "accepted {source}");
    }
    Ok(())
}

/// Actual and embedded Python parsers retain their configuration capability after renaming.
#[test]
fn auxiliary_toml_codecs_require_exact_reviews() -> Result<()> {
    for (path, source) in [
        ("tools/input.py", "import tomllib as schema; schema.loads(data)"),
        ("tools/input.py", "from tomllib import loads as decode"),
        ("tools/input.py", "import tomli as schema"),
        ("tools/input.py", "import toml as schema"),
        ("installer/worker/input.mjs", "import {parse as decode} from 'toml'"),
        ("installer/worker/input.mjs", "export const inspect = `import tomllib as schema`"),
    ] {
        let (mut repository, mut policy) = fixture()?;
        repository.architecture.owners[0].auxiliary_roots.push("tools".into());
        fixtures::write(repository.root(), path, source)?;
        assert!(check(&repository, &policy).is_err(), "accepted {source}");
        register(&repository, &mut policy, path, source)?;
        assert_eq!(policy.records[0].capabilities, BTreeSet::from([access::Capability::Codec]));
        check(&repository, &policy)?;
    }
    assert!(access::findings("const source = 'Cargo.toml'", false)?.is_empty());
    let maintenance =
        access::findings(include_str!("../../../../installer/tests/maintenance.ps1"), false)?;
    assert_eq!(maintenance, BTreeSet::from([access::Capability::Document]));
    Ok(())
}

#[test]
fn every_auxiliary_language_rejects_quoted_alias_native_and_browser_routes() -> Result<()> {
    for extension in access::EXTENSIONS.iter().filter(|extension| **extension != "rs") {
        for source in [
            "const { localStorage: renamed } = window; renamed.setItem(key,value)",
            "window['sessionStorage'].setItem(key,value)",
            "const config = process['env'];",
            "GetEnvironmentVariableW(key,value,length)",
            "$env:SUGAR_SETTING",
            "import Store from 'electron-store';",
            "RegSetValueExW(key,value)",
            "fs.writeFile('startup.local.toml', bytes)",
        ] {
            let (repository, policy) = fixture()?;
            fixtures::write(
                repository.root(),
                &format!("installer/worker/bypass.{extension}"),
                source,
            )?;
            assert!(check(&repository, &policy).is_err(), "accepted {extension}: {source}");
        }
    }
    Ok(())
}

#[test]
fn backend_dependency_identity_rejects_renamed_optional_target_and_test_routes() -> Result<()> {
    for backend in ["config", "figment", "confy", "winreg", "directories-next", "toml"] {
        for kind in [None, Some("dev"), Some("build")] {
            let (mut repository, policy) = fixture()?;
            let dependency = serde_json::from_value(serde_json::json!({
                "name": backend, "rename": "innocent", "kind": kind, "path": null,
                "optional": true, "target": "cfg(target_os = \"another-platform\")",
            }))
            .map_err(|cause| {
                Failure::external("decode aliased optional target dependency", cause)
            })?;
            repository.workspace.packages[0].dependencies.push(dependency);
            assert!(check(&repository, &policy).is_err());
        }
    }
    Ok(())
}

#[test]
fn reviewed_sources_require_exact_owner_capabilities_bytes_and_live_access() -> Result<()> {
    for variant in 0..9 {
        let (repository, mut policy) = fixture()?;
        register(
            &repository,
            &mut policy,
            "crates/host/src/owned.rs",
            "fn f(){ toml::from_str(input); }",
        )?;
        check(&repository, &policy)?;
        match variant {
            0 => policy.schema_version = 2,
            1 => policy.owner = "missing".into(),
            2 => policy.records[0].owner = "missing".into(),
            3 => policy.records[0].source_sha256 = "stale".into(),
            4 => policy.records[0].capabilities.clear(),
            5 => policy.records[0].rationale.clear(),
            6 => {
                policy.records[0].capabilities.insert(access::Capability::Environment);
            }
            7 => {
                fixtures::write(repository.root(), &policy.records[0].path, "fn harmless(){}")?;
                policy.records[0].source_sha256 =
                    fingerprint(repository.root(), &[policy.records[0].path.clone()])?;
            }
            _ => {
                let record = &policy.records[0];
                policy.records.push(Record {
                    path: record.path.clone(),
                    owner: record.owner.clone(),
                    source_sha256: record.source_sha256.clone(),
                    capabilities: record.capabilities.clone(),
                    rationale: record.rationale.clone(),
                });
            }
        }
        let failure = check(&repository, &policy)
            .err()
            .ok_or_else(|| Failure::Policy("Invalid configuration review accepted".into()))?
            .to_string();
        if variant == 7 {
            assert!(failure.contains(&policy.records[0].path), "{failure}");
        }
    }
    Ok(())
}

#[test]
fn policy_loading_rejects_unknown_schema_fields_and_missing_records() -> Result<()> {
    let (repository, _) = fixture()?;
    for source in [
        "schema_version=2\nowner='host'\nrecords=[]",
        "schema_version=1\nowner='host'\nsurfaces=[]\nrecords=[]\nextra=true",
        "schema_version=1\nowner='host'\nsurfaces=[]",
        "schema_version=1\nowner='host'\n[[records]]\npath='bad'",
    ] {
        fixtures::write(repository.root(), "policy.toml", source)?;
        assert!(crate::policy::load::<Policy>(repository.root(), "policy.toml").is_err());
    }
    Ok(())
}
#[test]
fn raw_public_types_aliases_and_reexports_cannot_escape_validated_configuration() -> Result<()> {
    for source in [
        "pub use toml::Value as Settings;",
        "pub use std::fs::*;",
        "pub fn settings() -> toml::Value { body() }",
        "use serde_json::Value as Hidden; type Indirect=Hidden; pub fn get()->Result<Indirect,Error>{body()}",
        "pub struct Settings { pub file: std::fs::File }",
        "impl Settings { pub fn file(&self)->&std::fs::File { body() } }",
        "#[cfg(unix)] pub type Settings=std::collections::BTreeMap<String,String>;",
        "pub enum Settings { Raw(toml::Value) }",
        "pub const SETTINGS: fn() -> toml::Value = read;",
        "pub static SETTINGS: Option<std::fs::File> = None;",
        "pub trait Settings { fn raw(&self) -> toml::Value; }",
        "macro_rules! raw { () => { pub fn settings() -> toml::Value { body() } } }",
        "macro_rules! raw { ($t:ty) => { pub fn settings() -> $t { body() } } } raw!(toml::Value);",
    ] {
        assert!(!public_exports(source)?, "accepted {source}");
    }
    assert!(public_exports(
        "struct Settings { value: toml::Value } impl Settings { pub fn locale(&self)->Locale { body() } }"
    )?);
    Ok(())
}
#[test]
fn reviewed_source_cannot_authorize_a_second_document_codec_or_persistent_backend() -> Result<()> {
    for (path, source) in [
        ("crates/feature/src/lib.rs", "fn f(){toml::from_str(input);}"),
        ("crates/feature/src/lib.rs", "fn f(){confy::load(name);}"),
        ("crates/feature/src/input.py", "import tomllib as schema; schema.loads(input)"),
    ] {
        let (mut repository, mut policy) = fixture()?;
        repository.architecture.owners.push(crate::policy::Owner {
            package: "feature".into(),
            root: "crates/feature".into(),
            auxiliary_roots: vec![],
            auxiliary_files: vec![],
            layer: "application".into(),
            concern: "Feature consumer".into(),
        });
        fixtures::write(repository.root(), path, source)?;
        policy.records = vec![Record {
            path: path.into(),
            owner: "feature".into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            capabilities: access::findings(source, path.ends_with(".rs"))?,
            rationale: "An exact review is not a second owner".into(),
        }];
        assert!(check(&repository, &policy).is_err());
    }
    Ok(())
}

/// SugarCubes package metadata has one finite external reader, without a general worker exemption.
#[test]
fn external_package_codec_cannot_authorize_another_worker_document_reader() -> Result<()> {
    let (mut repository, mut policy) = fixture()?;
    repository.architecture.owners[0].auxiliary_roots.clear();
    repository.architecture.owners.push(crate::policy::Owner {
        package: "feature".into(),
        root: "crates/feature".into(),
        auxiliary_roots: vec!["installer/worker".into()],
        auxiliary_files: vec![],
        layer: "application".into(),
        concern: "Supported external package metadata".into(),
    });
    for (path, admitted) in [
        ("installer/worker/dependency-inspection.mjs", true),
        ("installer/worker/another.mjs", false),
    ] {
        let source = "export const inspect = `import tomllib as schema`";
        fixtures::write(repository.root(), path, source)?;
        policy.records.push(Record {
            path: path.into(),
            owner: "feature".into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            capabilities: BTreeSet::from([access::Capability::Codec]),
            rationale: "External package metadata reader".into(),
        });
        assert_eq!(check(&repository, &policy).is_ok(), admitted);
    }
    Ok(())
}
#[test]
fn native_sources_require_unique_architecture_ownership() -> Result<()> {
    let (mut repository, policy) = fixture()?;
    fixtures::write(repository.root(), "native/capture/bridge.cc", "int harmless();")?;
    assert!(check(&repository, &policy).is_err());
    repository.architecture.owners[0].auxiliary_roots.push("native/capture".into());
    check(&repository, &policy)?;
    repository.architecture.owners.push(crate::policy::Owner {
        package: "foreign".into(),
        root: "crates/foreign".into(),
        auxiliary_roots: vec!["native/capture".into()],
        auxiliary_files: vec![],
        layer: "adapter".into(),
        concern: "Conflicting ownership".into(),
    });
    assert!(check(&repository, &policy).is_err());
    assert!(repository.architecture.owners.pop().is_some());
    repository.architecture.owners[0].auxiliary_roots.pop();
    assert!(check(&repository, &policy).is_err());
    Ok(())
}

/// Exercise the actual public-contract scanner against an isolated crate, not a source approximation.
fn public_exports(source: &str) -> Result<bool> {
    let repository = Repository::new()?;
    fixtures::write(repository.root(), "crates/host/src/lib.rs", source)?;
    exports::check(&repository.root().join("crates/host"), &[])
}

#[test]
fn cross_module_private_aliases_cannot_export_raw_configuration_types() -> Result<()> {
    let repository = Repository::new()?;
    fixtures::write(
        repository.root(),
        "crates/host/src/lib.rs",
        "mod hidden; pub fn expose() -> hidden::Hidden { body() }",
    )?;
    fixtures::write(
        repository.root(),
        "crates/host/src/hidden.rs",
        "pub(crate) type Hidden = toml::Value;",
    )?;
    assert!(!exports::check(&repository.root().join("crates/host"), &[])?);
    Ok(())
}

#[test]
fn loose_and_inactive_sources_reject_escaped_access_with_exact_ownership() -> Result<()> {
    for (path, source) in [
        ("tools/loose.CS", r"Environment.GetEnvironmentVariable(name)"),
        ("native/loose.CMAKE", "$ENV{SETTING}"),
        ("installer/worker/loose.PS1", "$EnV:SETTING"),
        ("installer/worker/node_modules/bypass.TS", r"process['e\u006ev']"),
        ("crates/host/inactive/extra.TS", "window['local'+'Storage']"),
    ] {
        let (mut repository, policy) = fixture()?;
        repository.architecture.owners[0].auxiliary_files.push(path.into());
        fixtures::write(repository.root(), path, source)?;
        assert!(!access::findings(source, false)?.is_empty(), "missed {source}");
        let expected = if access::findings(source, false)?.contains(&access::Capability::Backend) {
            "competing persistent settings backend"
        } else {
            "unowned access"
        };
        assert!(
            check(&repository, &policy)
                .err()
                .is_some_and(|error| error.to_string().contains(expected)),
            "accepted {path}"
        );
    }
    Ok(())
}

#[test]
fn nested_script_manifests_cannot_alias_independent_settings_backends() -> Result<()> {
    for kind in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        for (binding, version) in
            [("electron-store", "=10.0.0"), ("ordinary", "npm:electron-store@10.0.0")]
        {
            let (repository, policy) = fixture()?;
            fixtures::write(
                repository.root(),
                "installer/worker/nested/package.json",
                &serde_json::json!({kind:{binding:version}}).to_string(),
            )?;
            assert!(check(&repository, &policy).err().is_some_and(|error| {
                error.to_string().contains("independent script settings backend")
            }));
        }
    }
    Ok(())
}

#[test]
fn missing_source_reviews_report_every_affected_owner_path() -> Result<()> {
    let (repository, policy) = fixture()?;
    let paths = ["crates/host/src/first.rs", "installer/worker/second.ts"];
    fixtures::write(repository.root(), paths[0], r#"fn f(){std::env::var_os("SETTING");}"#)?;
    fixtures::write(repository.root(), paths[1], "process.env.SETTING")?;
    let error = check(&repository, &policy)
        .err()
        .ok_or_else(|| Failure::Policy("Unreviewed environment access was accepted".into()))?;
    for path in paths {
        assert!(error.to_string().contains(path), "{error}");
    }
    Ok(())
}
