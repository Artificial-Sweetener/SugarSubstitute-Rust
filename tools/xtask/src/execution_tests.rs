// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify execution ownership through real source fixtures and deliberately invalid policy.

use crate::{
    error::Result,
    execution::{Policy, check},
    execution_source::findings,
    fixtures::{Repository, write},
};

/// Reject direct, aliased, grouped, wildcard, function-pointer and macro-based thread creation.
#[test]
fn raw_threads_cannot_hide_behind_rust_import_or_macro_forms() -> Result<()> {
    for source in [
        "fn job() { std::thread::spawn(|| {}); }",
        "use std::thread as work; fn job() { work::spawn(|| {}); }",
        "use std::{thread::{Builder as Worker}}; fn job() { Worker::new().spawn(|| {}); }",
        "use std::thread::*; fn job() { spawn(|| {}); }",
        "use std as system; use system::thread as work; fn job() { work::scope(|_| {}); }",
        "use std::*; fn job() { thread::spawn(|| {}); }",
        "fn job() { let create = std::thread::spawn; create(|| {}); }",
        "macro_rules! job { () => { ::std::thread::spawn(|| {}) }; }",
        "invoke!(std::thread::spawn(|| {}));",
        "macro_rules! job { () => { use std::{thread as work}; work::spawn(|| {}); }; }",
        "macro_rules! job { () => { use std as platform; platform::thread::spawn(|| {}); }; }",
        "macro_rules! job { () => { use std::*; thread::spawn(|| {}); }; }",
        "macro_rules! launch { ($r:ident,$m:ident,$f:ident)=>{$r::$m::$f(||{});}; } fn job(){launch!(std,thread,spawn);}",
        "macro_rules! launch { ($r:ident,$m:ident,$f:ident)=>{$r::$m::$f(||{});}; } fn job(){launch!(r#std,r#thread,spawn);}",
        "use std as system; macro_rules! launch { ($r:ident,$m:ident,$f:ident)=>{$r::$m::$f(||{});}; } fn job(){launch!(system,thread,spawn);}",
        "mod harmless {} mod local { use crate::harmless as std; } fn job(){ ::std::thread::spawn(||{}); }",
        "mod harmless {} mod first { use std as system; fn job(){system::thread::spawn(||{});} } mod second {use crate::harmless as system;}",
        "#[cfg(target_os = \"linux\")] fn job() { std::thread::spawn(|| {}); }",
        "#[cfg(test)] mod tests { fn job() { std::thread::spawn(|| {}); } }",
    ] {
        assert!(!findings(source)?.is_empty(), "accepted {source}");
    }
    Ok(())
}

/// Reject async runtimes, pools, detached spawning and operating-system thread entry points.
#[test]
fn scheduling_capabilities_include_async_pools_and_native_ffi() -> Result<()> {
    for source in [
        "#[tokio::main] async fn main() {}",
        "use tokio as jobs; async fn job() { jobs::spawn(async {}); }",
        "extern crate tokio as jobs; fn job() { jobs::runtime::Runtime::new(); }",
        "fn job() { rayon::scope(|_| {}); }",
        "fn job() { async_std::task::spawn(async {}); }",
        "fn job() { let pool = threadpool::ThreadPool::new(2); }",
        "fn job() { handle.spawn_blocking(|| {}); }",
        "fn job() { context.spawn_local(async {}); }",
        "fn job() { unsafe { windows::Win32::System::Threading::CreateThread(); } }",
        "use libc::pthread_create as launch; fn job() { unsafe { launch(); } }",
        "fn job() { let (send, receive) = std::sync::mpsc::channel(); }",
        "fn job() { gtk::glib::MainContext::default().spawn(async {}); }",
        "fn job() { unsafe { windows::Win32::System::Threading::CreateThreadpoolTimer(); } }",
        "fn job() { unsafe { windows::Win32::System::Threading::CreateThreadpoolWait(); } }",
        "fn job() { unsafe { libc::thrd_create(); } }",
        "unsafe extern \"C\" { #[link_name=\"pthread_create\"] fn launch(); } fn job(){unsafe {launch();}}",
    ] {
        assert!(!findings(source)?.is_empty(), "accepted {source}");
    }
    Ok(())
}

/// Confine Windows wait callbacks and their aliases to the registered execution owner.
#[test]
fn native_wait_registration_cannot_escape_execution_ownership() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    for source in [
        "fn wait() { unsafe { windows::Win32::System::Threading::RegisterWaitForSingleObject(); } }",
        "use windows::Win32::System::Threading::RegisterWaitForSingleObject as register; fn wait() { unsafe { register(); } }",
        "unsafe extern \"system\" { #[link_name = \"RegisterWaitForSingleObject\"] fn register(); } fn wait() { unsafe { register(); } }",
        "macro_rules! wait { () => { unsafe { RegisterWaitForSingleObject(); } }; }",
        "fn wait() { unsafe { windows::Win32::System::Threading::SetThreadpoolWaitEx(); } }",
        "use windows::Win32::System::Threading::SetThreadpoolWaitEx as arm; fn wait() { unsafe { arm(); } }",
        "unsafe extern \"system\" { #[link_name = \"SetThreadpoolWaitEx\"] fn arm(); }",
        "#[cfg(test)] mod tests { fn wait() { unsafe { r#RegisterWaitForSingleObject(); } } }",
    ] {
        assert!(!findings(source)?.is_empty(), "accepted {source}");
        write(fixture.root(), "crates/host/src/waits.rs", source)?;
        let error = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(error.contains("outside the execution owner"), "unexpected result: {error}");
    }
    assert!(
        findings("const NOTE: &str = \"RegisterWaitForSingleObject SetThreadpoolWaitEx\";")?
            .is_empty()
    );
    Ok(())
}

/// Preserve language-level async, subprocess ownership, synchronization and inert prose.
#[test]
fn ordinary_logic_and_process_creation_are_not_thread_scheduling() -> Result<()> {
    let source = r#"
        // std::thread::spawn and tokio::spawn are documented examples.
        const EXAMPLE: &str = "std::thread::spawn(|| {})";
        struct State { blocking: bool }
        fn inspect(state: State) { assert!(!state.blocking); tracing::debug!(blocking = state.blocking); }
        use std::{process::Command, sync::Mutex, time::Instant};
        use super::*;
        async fn job() { other().await; }
        fn launch() { Command::new("server").spawn(); }
        fn paint() { window.request_redraw(); }
    "#;
    assert!(findings(source)?.is_empty());
    assert!(findings("fn broken( {").is_err());
    Ok(())
}

/// Forbid scheduling while no execution adapter has been implemented or registered.
#[test]
fn repository_gate_rejects_ad_hoc_scheduling_without_an_owner() -> Result<()> {
    let fixture = Repository::new()?;
    let policy = Policy { schema_version: 1, owner: None };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), "crates/host/src/jobs.rs", "fn job() { std::thread::spawn(|| {}); }")?;
    let error = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(error.contains("outside the execution owner"), "unexpected result: {error}");
    Ok(())
}

/// Dependency aliases and unused dev/target-specific runtimes still violate ownership.
#[test]
fn dependency_identity_cannot_hide_an_unused_or_renamed_runtime() -> Result<()> {
    for kind in [None, Some("dev"), Some("build")] {
        let mut fixture = Repository::new()?;
        fixture.workspace.packages[0].dependencies.push(
            serde_json::from_value(
                serde_json::json!({"name":"tokio", "rename":"background", "kind":kind,
                "path":null, "target":"cfg(target_os = \"linux\")"}),
            )
            .map_err(|error| {
                crate::error::Failure::external("construct dependency fixture", error)
            })?,
        );
        let policy = Policy { schema_version: 1, owner: None };
        let error = check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(error.contains("scheduler dependency tokio"), "unexpected result: {error}");
    }
    Ok(())
}

/// Require one actual dedicated adapter rather than allowing stale owners or host-wide waivers.
#[test]
fn execution_owner_must_be_registered_at_the_adapter_boundary() -> Result<()> {
    let mut fixture = Repository::new()?;
    let mut policy = Policy { schema_version: 1, owner: Some("missing".into()) };
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    policy.owner = Some("host".into());
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    fixture.architecture.layers.insert(1, "adapter".into());
    fixture.architecture.owners[0].layer = "adapter".into();
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    fixture.architecture.owners[0].concern = "Application execution and task lifetime".into();
    write(fixture.root(), "crates/host/src/jobs.rs", "fn job() { std::thread::spawn(|| {}); }")?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    Ok(())
}

/// Keep the owner allowance local; callers must use its ports rather than raw scheduler APIs.
#[test]
fn registered_owner_does_not_exempt_other_runtime_crates() -> Result<()> {
    let mut fixture = Repository::new()?;
    fixture.architecture.layers.insert(1, "adapter".into());
    fixture.architecture.owners.push(crate::policy::Owner {
        package: "execution".into(),
        root: "crates/execution".into(),
        layer: "adapter".into(),
        concern: "Application execution and task lifetime".into(),
    });
    fixture.workspace.packages.push(
        serde_json::from_value(serde_json::json!({
            "name":"execution", "id":"fixture-execution",
            "manifest_path":fixture.root().join("crates/execution/Cargo.toml"), "dependencies":[]
        }))
        .map_err(|error| crate::error::Failure::external("construct owner fixture", error))?,
    );
    write(
        fixture.root(),
        "crates/execution/src/lib.rs",
        "fn job() { std::thread::spawn(|| {}); }",
    )?;
    let policy = Policy { schema_version: 1, owner: Some("execution".into()) };
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), "crates/host/src/jobs.rs", "fn job() { std::thread::spawn(|| {}); }")?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    Ok(())
}

/// Reject policy fields that would silently introduce test or platform exemptions.
#[test]
fn execution_policy_rejects_future_schemas_and_undeclared_exemptions() -> Result<()> {
    let fixture = Repository::new()?;
    for source in [
        "schema_version = 2",
        "schema_version = 1\nallow_tests = true",
        "schema_version = 1\nowners = ['host']",
        "schema_version = 1\nallowed_paths = ['crates/host']",
    ] {
        write(fixture.root(), "execution.toml", source)?;
        assert!(crate::policy::load::<Policy>(fixture.root(), "execution.toml").is_err());
    }
    Ok(())
}
