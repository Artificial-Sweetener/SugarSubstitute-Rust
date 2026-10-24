// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adversarial fixtures qualify source bypass, exact boundary review and missing coverage rejection.
use super::*;
use crate::{
    fixtures::{self, Repository},
    policy::Owner,
};

fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].auxiliary_roots.push("worker".into());
    repository.architecture.layers.push("adapter".into());
    repository.architecture.owners.push(Owner {
        package: "storage".into(),
        root: "crates/storage".into(),
        auxiliary_roots: vec![],
        layer: "adapter".into(),
        concern: "Storage adapter".into(),
    });
    repository.workspace.packages.push(serde_json::from_value(serde_json::json!({"name":"storage","id":"fixture-storage","manifest_path":repository.root().join("crates/storage/Cargo.toml"),"dependencies":[]})).map_err(|cause| Failure::external("construct diagnostic fixture", cause))?);
    fixtures::write(repository.root(), "worker/package.json", "{}")?;
    let mut records = Vec::new();
    for (path, owner, disposition, text) in [
        (
            "crates/host/src/diagnostics.rs",
            "host",
            Disposition::Routing,
            "fn install() { let sink = std::io::stderr(); }",
        ),
        (
            "crates/storage/src/diagnostics.rs",
            "storage",
            Disposition::Storage,
            "pub fn append() {}",
        ),
        ("worker/main.ts", "host", Disposition::Routing, "console.log = route"),
    ] {
        fixtures::write(repository.root(), path, text)?;
        records.push(Record {
            path: path.into(),
            owner: owner.into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            disposition,
            rationale: "Actual supported fixture boundary".into(),
        });
    }
    let policy = Policy {
        schema_version: 1,
        owner: "host".into(),
        routing_entry: "crates/host/src/diagnostics.rs".into(),
        storage_owner: "storage".into(),
        storage_entry: "crates/storage/src/diagnostics.rs".into(),
        worker_entry: "worker/main.ts".into(),
        required_roots: vec!["worker".into()],
        script_manifests: vec!["worker/package.json".into()],
        records,
    };
    Ok((repository, policy))
}

#[test]
fn aliases_wrappers_macro_fragments_platforms_tests_and_targets_cannot_create_rust_sinks()
-> Result<()> {
    for source in [
        "fn main() -> Result<(), Error> { Ok(()) }",
        "fn f() { println!(\"runtime\"); }",
        "fn f() { std::eprintln!(\"runtime\"); }",
        "use std::io::stderr as hidden; fn wrapper() { hidden(); }",
        "pub use std::io as output; fn wrapper() { output::stdout(); }",
        "use std::io::{self}; fn f() { io::stderr(); }",
        "use std::io::*; fn f() { stdout(); }",
        "use tracing_subscriber as other; fn f() { other::fmt(); }",
        "use tracing::subscriber::set_global_default as install; fn f() { install(x); }",
        "pub use tracing::Dispatch as Raw;",
        "pub use tracing as Raw;",
        "pub extern crate tracing as Raw;",
        "pub use tracing::subscriber as Raw;",
        "pub use tracing_subscriber::fmt as Raw;",
        "unsafe extern \"C\" {pub fn raw() -> tracing::Dispatch;}",
        "use sugarsubstitute_supervision::diagnostic_storage::operational as logs; fn f(){logs::append(x,y,z);}",
        "pub type Raw = tracing::Dispatch;",
        "fn wrapper() { std::io::stdout() }",
        "macro_rules! output { () => { eprintln!(\"runtime\") } }",
        "macro_rules! output { ($s:ident) => { $s::io::stderr() } }",
        "macro_rules! output { ($a:ident,$b:ident,$c:ident) => {$a::$b::$c()} } fn f(){output!(std,io,stderr)}",
        "#[cfg(test)] fn fixture() { std::println!(\"bypass\"); }",
        "#[cfg(unix)] fn platform() { tracing_subscriber::fmt(); }",
        "unsafe extern \"system\" { #[link_name=\"OutputDebugStringW\"] fn hidden(); }",
        "fn f(){ let destination = \"independent.log\"; }",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/examples/bypass.rs", source)?;
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err(),
            "accepted {source}"
        );
    }
    Ok(())
}
#[test]
fn dependency_identity_aliases_and_raw_owner_exports_are_rejected() -> Result<()> {
    let (mut repository, policy) = fixture()?;
    repository.workspace.packages[0].dependencies.push(
        serde_json::from_value(
            serde_json::json!({"name":"env_logger","rename":"innocent","kind":"dev","path":null}),
        )
        .map_err(|cause| Failure::external("construct aliased logger", cause))?,
    );
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    let dependencies = repository.workspace.packages[0].dependencies.as_slice();
    assert!(!source::rust("fn f(){ innocent::init(); }", dependencies)?.is_empty());
    for public in [
        "pub use tracing::Dispatch as Raw;",
        "pub type Raw = tracing::Dispatch;",
        "use tracing::Dispatch as Internal; pub fn raw() -> Internal { unreachable!() }",
        "use tracing::Dispatch as Internal; type Alias = Internal; pub use Alias as Raw;",
        "pub fn raw() -> impl tracing::Subscriber { unreachable!() }",
        "pub struct Raw { pub dispatch: tracing::Dispatch }",
        "pub enum Raw { Dispatch(tracing::Dispatch) }",
        "impl Route { pub fn raw(&self) -> tracing::Dispatch { unreachable!() } }",
        "pub trait Route { fn raw(&self) -> tracing::Dispatch; }",
        "pub trait Route: tracing::Subscriber { type Raw: tracing::Subscriber; }",
        "pub static RAW: tracing::Dispatch = value;",
        "pub union Raw { pub dispatch: tracing::Dispatch }",
        "pub fn raw() -> opaque_type!() { unreachable!() }",
    ] {
        assert!(source::public_raw_exports(public)?);
    }
    assert!(!source::public_raw_exports(
        "fn private() -> impl tracing::Subscriber { unreachable!() }"
    )?);
    assert!(!source::public_raw_exports(
        "pub fn run() {} fn private() -> impl tracing::Subscriber { unreachable!() }"
    )?);
    Ok(())
}
#[test]
fn cross_module_aliases_and_associated_types_cannot_export_raw_routing_authority() -> Result<()> {
    let (repository, policy) = fixture()?;
    let boundary = repository.root().join("crates/host/src/lib.rs");
    fixtures::write(
        repository.root(),
        "crates/host/src/lib.rs",
        "mod tracing {} pub use ::tracing as Public;",
    )?;
    assert!(
        source::crate_public_raw_exports(&repository.root().join("crates/host"), &[])?
            .contains(&boundary)
    );
    fixtures::write(
        repository.root(),
        "crates/host/src/lib.rs",
        "mod hidden; pub use hidden::Alias as Public;",
    )?;
    fixtures::write(
        repository.root(),
        "crates/host/src/hidden.rs",
        "pub type Alias = tracing::Dispatch;",
    )?;
    assert!(
        source::crate_public_raw_exports(&repository.root().join("crates/host"), &[])?
            .contains(&boundary)
    );
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    fixtures::write(
        repository.root(),
        "crates/host/src/hidden.rs",
        "use tracing::*; pub type Alias = Dispatch;",
    )?;
    assert!(
        source::crate_public_raw_exports(&repository.root().join("crates/host"), &[])?
            .contains(&boundary)
    );
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    fixtures::write(
        repository.root(),
        "crates/host/src/lib.rs",
        "mod hidden; pub fn raw() -> <hidden::Holder as hidden::Contract>::Output { unreachable!() }",
    )?;
    fixtures::write(
        repository.root(),
        "crates/host/src/hidden.rs",
        "pub struct Holder; pub trait Contract {type Output;} impl Contract for Holder {type Output=tracing::Dispatch;}",
    )?;
    assert!(
        source::crate_public_raw_exports(&repository.root().join("crates/host"), &[])?
            .contains(&boundary)
    );
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    Ok(())
}
#[test]
fn every_auxiliary_language_checks_console_native_imports_and_persistent_sink_names() -> Result<()>
{
    for extension in EXTENSIONS.iter().filter(|extension| **extension != "rs") {
        for source in [
            "const { log: hidden } = console; hidden(value)",
            "const sink = console['warn']; sink(value)",
            "process['stdout'].write(value)",
            "import { createWriteStream as open } from 'node:fs'; open(path)",
            "std::fprintf(stderr, value)",
            "OutputDebugStringW(value)",
            "open('separate.log')",
        ] {
            let (repository, policy) = fixture()?;
            fixtures::write(repository.root(), &format!("worker/bypass.{extension}"), source)?;
            assert!(
                check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                    .is_err(),
                "accepted {extension}: {source}"
            );
        }
    }
    Ok(())
}
#[test]
fn module_and_reexported_function_with_the_same_name_resolve_without_recursion() -> Result<()> {
    let (repository, policy) = fixture()?;
    let root = repository.root().join("crates/host");
    fixtures::write(
        repository.root(),
        "crates/host/src/lib.rs",
        "mod entry; pub use entry::entry;",
    )?;
    fixtures::write(repository.root(), "crates/host/src/entry.rs", "pub fn entry() -> u64 { 42 }")?;
    assert!(source::crate_public_raw_exports(&root, &[])?.is_empty());
    check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
    fixtures::write(
        repository.root(),
        "crates/host/src/entry.rs",
        "pub fn entry() -> tracing::Dispatch { unreachable!() }",
    )?;
    assert!(source::crate_public_raw_exports(&root, &[])?.contains(&root.join("src/entry.rs")));
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    Ok(())
}
#[test]
fn unknown_schemas_stale_reviews_missing_coverage_and_false_owner_dispositions_reject() -> Result<()>
{
    let (mut repository, mut policy) = fixture()?;
    check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
    policy.schema_version = 2;
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    policy.schema_version = 1;
    policy.records[0].source_sha256 = "sha256:stale".into();
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    policy.records[0].source_sha256 =
        fingerprint(repository.root(), &[policy.routing_entry.clone()])?;
    policy.records[0].disposition = Disposition::Developer;
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    policy.records[0].disposition = Disposition::Routing;
    repository.architecture.owners[0].auxiliary_roots.clear();
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    let source = "schema_version=1\nowner='host'\nunknown=true";
    assert!(toml::from_str::<Policy>(source).is_err());
    Ok(())
}
#[test]
fn ordinary_file_and_process_apis_remain_available_to_their_feature_owners() -> Result<()> {
    assert!(
        source::rust("fn main(session: &mut Session) -> Result<State, Error> {}", &[])?.is_empty()
    );
    let source = "fn f() { let file = std::fs::File::open(path); let child = std::process::Command::new(executable).stdout(std::process::Stdio::piped()); }";
    assert!(source::rust(source, &[])?.is_empty());
    assert!(
        source::rust("fn f(){ tracing::info!(operation=\"startup\",request_id=42); }", &[])?
            .is_empty()
    );
    assert!(
        source::rust("fn f(){ let text=\"std::io::stdout and eprintln! are prose\"; }", &[])?
            .is_empty()
    );
    Ok(())
}

#[test]
fn auxiliary_sources_cannot_disappear_by_removing_policy_and_architecture_coverage() -> Result<()> {
    let (repository, policy) = fixture()?;
    fixtures::write(
        repository.root(),
        "installer/package.json",
        r#"{"dependencies":{"logger":"npm:pino@1.0.0"}}"#,
    )?;
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    std::fs::remove_file(repository.root().join("installer/package.json"))
        .map_err(|cause| Failure::external("remove dependency fixture", cause))?;
    fixtures::write(repository.root(), "native/unknown/sink.cc", "void output(){printf(value);}")?;
    assert!(
        check(repository.root(), &repository.architecture, &repository.workspace, &policy).is_err()
    );
    Ok(())
}

#[test]
fn script_logger_aliases_and_native_stdout_construction_cannot_hide_ownership() -> Result<()> {
    for package in [
        r#"{"dependencies":{"innocent":"npm:pino@1.0.0"}}"#,
        r#"{"optionalDependencies":{"winston":"1.0.0"}}"#,
        r#"{"devDependencies":{"debug":"1.0.0"}}"#,
    ] {
        assert!(source::script_dependencies(package).is_err());
    }
    assert!(source::script_dependencies(r#"{"dependencies":{"yaml":"2.9.0"}}"#).is_ok());
    assert!(
        !source::rust(
            "use windows::Win32::System::Console::GetStdHandle as hidden; fn f(){hidden(x);}",
            &[]
        )?
        .is_empty()
    );
    assert!(!source::rust(r#"fn f(){ std::fs::File::create("CONOUT$"); }"#, &[])?.is_empty());
    assert!(source::html(r#"<div id="console"></div>"#).is_empty());
    assert!(!source::html("<script>console.log(value)</script>").is_empty());
    Ok(())
}
