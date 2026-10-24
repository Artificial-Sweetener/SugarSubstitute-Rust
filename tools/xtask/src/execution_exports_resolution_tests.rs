// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify lexical aliases, Cargo renames and real multi-file public re-exports.

use crate::{
    error::{Failure, Result},
    execution::{Policy, check},
    execution_exports::findings,
    execution_exports_tests::inspect,
    fixtures::{Repository, write},
};

/// A module and re-exported function may share a name without growing alias paths forever.
#[test]
fn module_function_name_collisions_preserve_finite_public_resolution() -> Result<()> {
    let repository = Repository::new()?;
    write(repository.root(), "crates/host/src/lib.rs", "mod entry; pub use entry::entry;")?;
    write(repository.root(), "crates/host/src/entry.rs", "pub fn entry() {}")?;
    let root = repository.root().join("crates/host");
    assert!(findings(&root, &[])?.is_empty());
    write(
        repository.root(),
        "crates/host/src/entry.rs",
        "pub fn entry()->tokio::runtime::Runtime {loop{}}",
    )?;
    assert!(!findings(&root, &[])?.is_empty());
    Ok(())
}

/// Private aliases and nested public exports resolve back to the original capability.
#[test]
fn aliases_and_scheduler_globs_cannot_hide_public_types() -> Result<()> {
    for source in [
        "use tokio::runtime as rt; type Hidden = rt::Handle; pub type Escape = Hidden;",
        "mod private { use tokio::runtime::Handle as H; pub type Escape = H; } pub use private::Escape;",
        "mod private { pub use tokio::{runtime::{self as rt, Handle as H}}; } pub use private::rt as escape;",
        "mod first { pub use tokio::runtime::Handle as H; } mod second { use crate::first::H; pub fn raw() -> H { loop {} } }",
        "use tokio::runtime::*; pub fn raw() -> Handle { loop {} }",
        "use tokio::task::*; pub type Escape = JoinHandle<()>;",
        "mod child { pub use super::raw as jobs; } use tokio as raw;",
        "use tokio::runtime::Handle as H; type A<T> = (T, H); pub fn raw() -> A<()> { loop {} }",
        "mod harmless { pub struct Value; } use crate::harmless as tokio; pub use ::tokio::runtime::Handle;",
        "mod harmless { pub struct Value; } use crate::harmless as tokio; use ::tokio::runtime::Handle as H; pub type Escape = H;",
        "#[cfg(windows)] mod platform { type H = tokio::runtime::Handle; pub type Job = H; } #[cfg(unix)] mod platform { pub struct Job; }",
        "#[cfg(windows)] type H = tokio::runtime::Handle; #[cfg(unix)] type H = (); pub type Escape = H;",
        "#[cfg(windows)] type H = tokio::runtime::Handle; #[cfg(unix)] use std::string::String as H; pub type Escape = H;",
        "struct Private; trait Project { type Scheduler; } impl Project for Private { type Scheduler = tokio::runtime::Handle; } pub type Escape = <Private as Project>::Scheduler;",
        "struct Private; trait Project { type Scheduler; } impl Project for Private { type Scheduler = tokio::runtime::Handle; } type Hidden = <Private as Project>::Scheduler; pub type Escape = Hidden;",
    ] {
        assert!(!inspect(source)?.is_empty(), "accepted {source}");
    }
    let safe = "mod private { use tokio::runtime::Handle; struct State { raw: Handle } } mod contracts { pub struct Handle; pub fn typed() -> Handle { Handle } }";
    assert!(inspect(safe)?.is_empty());
    assert!(
        inspect("use tokio::runtime::*; pub struct Client; pub fn client() -> Client { Client }")?
            .is_empty()
    );
    assert!(inspect("pub fn typed<Handle>(value: Handle) -> Handle { value }")?.is_empty());
    assert!(inspect("mod harmless { pub struct Value; } use crate::harmless as tokio; pub fn typed() -> tokio::Value { loop {} }")?.is_empty());
    assert!(!inspect("mod harmless { pub struct Value; } use crate::harmless as tokio; pub fn raw() -> ::tokio::runtime::Handle { loop {} }")?.is_empty());
    assert!(!inspect("use tokio::runtime::Handle; pub trait Typed { type Value<Handle>; fn raw() -> Handle; }")?.is_empty());
    Ok(())
}

/// Canonical Cargo identities survive renaming and multiple source module boundaries.
#[test]
fn cargo_renames_and_external_module_reexports_are_rejected() -> Result<()> {
    let fixture = Repository::new()?;
    write(
        fixture.root(),
        "crates/host/src/lib.rs",
        "mod private; pub use crate::private::Escape;",
    )?;
    write(
        fixture.root(),
        "crates/host/src/private.rs",
        "use background::runtime::Handle as H; pub type Escape = H;",
    )?;
    let dependency = serde_json::from_value(
        serde_json::json!({"name":"tokio", "rename":"background", "kind":null, "path":null}),
    )
    .map_err(|error| Failure::external("construct renamed scheduler fixture", error))?;
    let errors = findings(&fixture.root().join("crates/host"), &[dependency])?;
    assert!(errors.iter().any(|error| error.contains("tokio::runtime::Handle")), "{errors:?}");
    write(
        fixture.root(),
        "crates/host/src/lib.rs",
        "#[path = \"native/worker.rs\"] mod platform; pub use crate::platform::Escape;",
    )?;
    write(
        fixture.root(),
        "crates/host/src/native/worker.rs",
        "pub type Escape = std::thread::JoinHandle<()>;",
    )?;
    assert!(!findings(&fixture.root().join("crates/host"), &[])?.is_empty());
    write(
        fixture.root(),
        "crates/host/src/lib.rs",
        "#[cfg_attr(windows, path = \"windows.rs\")] #[cfg_attr(unix, path = \"unix.rs\")] mod platform; pub use platform::Owned;",
    )?;
    write(fixture.root(), "crates/host/src/windows.rs", "pub struct Owned;")?;
    write(fixture.root(), "crates/host/src/unix.rs", "pub struct Owned;")?;
    write(fixture.root(), "crates/host/src/native/worker.rs", "struct Private;")?;
    write(fixture.root(), "crates/host/src/private.rs", "struct Private;")?;
    assert!(findings(&fixture.root().join("crates/host"), &[])?.is_empty());
    write(
        fixture.root(),
        "crates/host/src/windows.rs",
        "pub type Owned = std::thread::JoinHandle<()>;",
    )?;
    assert!(!findings(&fixture.root().join("crates/host"), &[])?.is_empty());
    write(fixture.root(), "crates/host/src/lib.rs", "#[path = \"lib.rs\"] mod recursive;")?;
    let error = findings(&fixture.root().join("crates/host"), &[])
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(error.contains("recursive module source"), "{error}");
    Ok(())
}

/// Owner registration permits its implementation but never permits its raw public API.
#[test]
fn repository_owner_gate_enforces_export_contracts() -> Result<()> {
    let mut fixture = Repository::new()?;
    fixture.architecture.layers.insert(1, "adapter".into());
    fixture.architecture.owners[0].layer = "adapter".into();
    fixture.architecture.owners[0].concern = "Application execution and task lifetime".into();
    let policy = Policy { schema_version: 1, owner: Some("host".into()) };
    write(fixture.root(), "crates/host/src/lib.rs", "fn work() { tokio::spawn(async {}); }")?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), "crates/host/src/lib.rs", "pub use tokio::runtime::Handle;")?;
    let error = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(error.contains("exports raw scheduling capabilities"), "{error}");
    Ok(())
}
