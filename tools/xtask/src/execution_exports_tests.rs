// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise owner export rejection while allowing private runtime state and typed execution ports.

use crate::{
    error::Result,
    execution_exports::findings,
    fixtures::{Repository, write},
};

/// Audit one independent source fixture through the same owner-boundary entry point.
pub(crate) fn inspect(source: &str) -> Result<Vec<String>> {
    let fixture = Repository::new()?;
    write(fixture.root(), "crates/host/src/lib.rs", source)?;
    findings(&fixture.root().join("crates/host"), &[])
}

/// Reject every declaration shape that can reveal scheduler values or callable entry points.
#[test]
fn declared_public_scheduler_capabilities_are_rejected() -> Result<()> {
    for source in [
        "pub use tokio::runtime::Runtime;",
        "pub use tokio;",
        "pub use tokio::runtime as scheduler;",
        "pub use tokio::runtime::*;",
        "pub use std::sync::*;",
        "pub use std::*;",
        "pub extern crate tokio as scheduler;",
        "pub type Worker = tokio::runtime::Handle;",
        "use tokio::task::JoinHandle as J; pub fn job() -> Option<J<()>> { loop {} }",
        "pub fn input(handle: &tokio::runtime::Handle) {}",
        "pub fn job<F: tokio::task::LocalFuture>(f: F) {}",
        "pub struct Worker<T = tokio::task::JoinSet<()>> { state: T }",
        "pub struct Worker { pub handle: tokio::runtime::Handle }",
        "pub struct Worker(pub std::thread::JoinHandle<()>);",
        "pub union Worker { pub handle: std::thread::Thread, private: usize }",
        "pub enum Worker { Active(tokio::runtime::Runtime) }",
        "pub trait Tasks: tokio::task::Spawn { fn take(&self) -> tokio::task::JoinHandle<()>; }",
        "pub trait Tasks { type Handle = tokio::runtime::Handle; }",
        "pub trait Tasks { const HANDLE: Option<tokio::runtime::Handle>; }",
        "pub trait Escape { const YIELD: fn() = std::thread::yield_now; }",
        "pub struct Owner; impl std::ops::Deref for Owner { type Target = tokio::runtime::Runtime; fn deref(&self) -> &Self::Target { loop {} } }",
        "pub union Owner { private: u8 } impl std::ops::Deref for Owner { type Target = tokio::runtime::Handle; fn deref(&self) -> &Self::Target { loop {} } }",
        "pub trait Expose { type Scheduler; fn raw() -> Self::Scheduler; } impl Expose for () { type Scheduler = tokio::runtime::Handle; fn raw() -> Self::Scheduler { loop {} } }",
        "pub struct Owner; impl std::ops::Deref for &Owner { type Target = tokio::runtime::Handle; fn deref(&self) -> &Self::Target { loop {} } }",
        "pub struct Owner; impl AsRef<tokio::runtime::Handle> for Owner { fn as_ref(&self) -> &tokio::runtime::Handle { loop {} } }",
        "pub struct Owner; impl Owner { pub fn runtime(&self) -> tokio::runtime::Handle { loop {} } }",
        "use std::thread::spawn as make; pub const RAW: fn() = make;",
        "pub static RAW: fn() = std::thread::yield_now;",
        "pub fn raw() -> fn() -> tokio::runtime::Handle { loop {} }",
        "pub fn raw() -> impl Iterator<Item = tokio::runtime::Handle> { loop {} }",
        "pub fn raw() -> Box<dyn Iterator<Item = std::sync::mpsc::Sender<()>>> { loop {} }",
        "pub type Completion<T> = <T as tokio::task::Join>::Output;",
        "pub use windows::Win32::System::Pipes::ConnectNamedPipe as connect;",
        "pub use windows::Win32::System::IO::CancelIoEx as cancel;",
        "pub use windows::Win32::System::IO::GetOverlappedResult as settle;",
        "pub use windows::Win32::System::IO::GetOverlappedResultEx as settle;",
        "unsafe extern \"system\" { #[link_name = \"CancelIoEx\"] pub fn cancel(); }",
        "pub use windows::Win32::System::Threading::CreateThreadpoolWork as launch;",
        "pub use windows::Win32::System::Threading::RegisterWaitForSingleObject as register;",
        "pub use windows::Win32::System::Threading::SetThreadpoolWaitEx as arm;",
        "unsafe extern \"system\" { #[link_name = \"RegisterWaitForSingleObject\"] pub fn register(); }",
        "unsafe extern \"system\" { pub fn SetThreadpoolWaitEx(); }",
        "unsafe extern \"C\" { pub fn raw() -> tokio::runtime::Handle; }",
        "unsafe extern \"C\" { #[link_name = \"CreateThread\"] pub fn launch(); }",
        "unsafe extern \"system\" { pub fn CreateThread(); }",
    ] {
        assert!(!inspect(source)?.is_empty(), "accepted {source}");
    }
    Ok(())
}

/// Private runtime state and standard synchronization do not leak scheduling capabilities.
#[test]
fn private_implementations_and_owned_ports_remain_valid() -> Result<()> {
    let source = r#"
        use std::{future::Future, sync::{Arc, Mutex, atomic::AtomicBool}, task::Waker};
        use tokio::runtime::Handle;
        type Raw = Handle;
        struct PrivateRuntime { handle: Raw, tasks: tokio::task::JoinSet<()> }
        impl PrivateRuntime { fn run(&self) { tokio::spawn(async {}); } }
        struct PrivateGeneric<T>(T);
        impl<T: tokio::task::Spawn> PrivateGeneric<T> { fn run(&self) {} }
        pub struct Shadow;
        impl<Shadow: tokio::task::Spawn> PrivateGeneric<Shadow> { fn run(&self) {} }
        trait Internal { type State; }
        impl Internal for PrivateGeneric<Shadow> { type State = tokio::runtime::Handle; }
        pub(crate) fn internal() -> Handle { loop {} }
        pub struct Scope { cancelled: Arc<AtomicBool> }
        pub struct OwnedJob<T> { state: Arc<Mutex<Option<T>>>, wake: Option<Waker> }
        pub trait ExecutionPort {
            fn submit<T, F: Future<Output = T>>(&self, scope: &Scope, work: F) -> OwnedJob<T>;
        }
        pub struct Client { handle: Handle }
        impl Client { pub fn submit<T, F: Future<Output = T>>(&self, work: F) -> OwnedJob<T> { loop {} } }
        pub const NOTE: &str = "tokio::runtime::Handle";
        pub const FIELDS: () = tracing::debug!(runtime = true, handle = true, thread = true, std = true, blocking = true);
        macro_rules! local_worker { () => { tokio::spawn(async {}); }; }
    "#;
    assert!(inspect(source)?.is_empty());
    Ok(())
}

/// Public signature macros require explicit analyzable types rather than a silent clean result.
#[test]
fn public_type_macros_fail_with_actionable_errors() -> Result<()> {
    for source in [
        "pub type Job = raw_type!();",
        "pub fn raw() -> raw_type!() { loop {} }",
        "pub struct Job { pub value: raw_type!() }",
        "macro_rules! raw_type { () => { tokio::runtime::Handle }; } type Hidden = raw_type!(); pub type Escape = Hidden;",
    ] {
        let errors = inspect(source)?;
        assert!(
            errors.iter().any(|error| error.contains("spell the execution type explicitly")),
            "{errors:?}"
        );
    }
    assert!(inspect("struct Private { value: raw_type!() }")?.is_empty());
    Ok(())
}

/// Exported transcribers are checked as paths and assembly, not identifier bags or matchers.
#[test]
fn exported_macro_scheduler_paths_and_fragments_are_rejected() -> Result<()> {
    for source in [
        "#[macro_export] macro_rules! io { () => { unsafe { ConnectNamedPipe(); CancelIoEx(); GetOverlappedResult(); } }; }",
        "#[macro_export] macro_rules! work { () => { tokio::spawn(async {}); }; }",
        "use tokio as jobs; #[macro_export] macro_rules! work { () => { jobs::spawn(async {}); }; }",
        "#[macro_export] macro_rules! work { ($r:ident,$m:ident,$f:ident) => { $r::$m::$f(||{}); }; }",
        "#[macro_export] macro_rules! work { ($path:path) => { $path::spawn(||{}); }; }",
        "mod private { #[macro_export] macro_rules! work { () => { std::thread::spawn(||{}); }; } }",
        "mod private { macro_rules! work { () => { tokio::spawn(async {}); }; } } pub use crate::private::work;",
        "#[macro_export] macro_rules! work { () => { use tokio::runtime as jobs; jobs::Runtime::new(); }; }",
        "#[macro_export] macro_rules! work { () => { use tokio as jobs; jobs::spawn(async {}); }; }",
        "use std::thread::spawn as make; #[macro_export] macro_rules! work { () => { make(||{}); }; }",
        "mod harmless { pub fn spawn() {} } use crate::harmless as tokio; #[macro_export] macro_rules! work { () => { ::tokio::spawn(async {}); }; }",
        "#[cfg_attr(feature = \"public\", macro_export)] macro_rules! work { () => { tokio::spawn(async {}); }; }",
        "fn helper() -> tokio::runtime::Handle { loop {} } #[macro_export] macro_rules! work { () => { $crate::helper() }; }",
        "fn helper() -> tokio::runtime::Handle { loop {} } #[macro_export] macro_rules! work { () => { $crate::helper }; }",
        "macro_rules! hidden { () => { std::thread::yield_now }; } pub const RAW: fn() = hidden!();",
        "macro_rules! second { () => { std::thread::yield_now }; } macro_rules! first { () => { second!() }; } pub const RAW: fn() = first!();",
        "macro_rules! second { () => { first!(); std::thread::yield_now }; } macro_rules! first { () => { second!() }; } pub const RAW: fn() = first!();",
        "pub const FIELD: () = tracing::debug!(thread = std::thread::yield_now);",
    ] {
        assert!(!inspect(source)?.is_empty(), "accepted {source}");
    }
    let harmless = r#"
        #[macro_export] macro_rules! fields {
            ($thread:expr, $std:expr) => { tracing::debug!(thread = $thread, std = $std, runtime = true, handle = true); };
        }
        #[macro_export] macro_rules! text { () => { "std::thread::spawn" }; }
        fn owned() -> () {}
        macro_rules! second { () => { first!() }; }
        macro_rules! first { () => { second!(); owned() }; }
        pub const SAFE: () = first!();
    "#;
    assert!(inspect(harmless)?.is_empty());
    Ok(())
}
