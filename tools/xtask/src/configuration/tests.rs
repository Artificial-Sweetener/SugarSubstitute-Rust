// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adversarial configuration access and strict review integrity fixtures.
use super::*;
use crate::fixtures::{self, Repository};

fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "adapter".into();
    repository.architecture.owners[0].auxiliary_roots.push("worker".into());
    fixtures::write(repository.root(), "worker/main.ts", "export {}")?;
    Ok((
        repository,
        Policy { schema_version: 1, owner: "host".into(), records: vec![], surfaces: vec![] },
    ))
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
            fixtures::write(repository.root(), &format!("worker/bypass.{extension}"), source)?;
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
        assert!(check(&repository, &policy).is_err());
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
        assert!(!exports::check(source)?, "accepted {source}");
    }
    assert!(exports::check(
        "struct Settings { value: toml::Value } impl Settings { pub fn locale(&self)->Locale { body() } }"
    )?);
    Ok(())
}
#[test]
fn reviewed_source_cannot_authorize_a_second_document_codec_or_persistent_backend() -> Result<()> {
    let (mut repository, mut policy) = fixture()?;
    repository.architecture.owners.push(crate::policy::Owner {
        package: "feature".into(),
        root: "crates/feature".into(),
        auxiliary_roots: vec![],
        layer: "application".into(),
        concern: "Feature consumer".into(),
    });
    let path = "crates/feature/src/lib.rs";
    for source in ["fn f(){toml::from_str(input);}", "fn f(){confy::load(name);}"] {
        fixtures::write(repository.root(), path, source)?;
        policy.records = vec![Record {
            path: path.into(),
            owner: "feature".into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            capabilities: access::findings(source, true)?,
            rationale: "An exact review is not a second owner".into(),
        }];
        assert!(check(&repository, &policy).is_err());
    }
    Ok(())
}
#[test]
fn native_sources_require_unique_explicit_surface_coverage() -> Result<()> {
    let (repository, mut policy) = fixture()?;
    fixtures::write(repository.root(), "native/capture/bridge.cc", "int harmless();")?;
    assert!(check(&repository, &policy).is_err());
    policy.surfaces.push(Surface { root: "native/capture".into(), owner: "host".into() });
    check(&repository, &policy)?;
    policy.surfaces.push(Surface { root: "native".into(), owner: "host".into() });
    assert!(check(&repository, &policy).is_err());
    assert!(policy.surfaces.pop().is_some());
    policy.surfaces[0].owner = "foreign".into();
    assert!(check(&repository, &policy).is_err());
    Ok(())
}
