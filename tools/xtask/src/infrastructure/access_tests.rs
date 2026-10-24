// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unreviewed construction, method effects, cross-language routes and raw public exports.
use super::{
    access,
    tests::{check, fixture},
    *,
};
use crate::fixtures;

/// Constructors remain restricted through aliases, UFCS, default builders and split macro paths.
#[test]
fn constructors_and_methods_cannot_hide_behind_rust_aliases_or_macros() -> Result<()> {
    for source in [
        "use std::fs::File as Disk; fn f(){Disk::open(p);}",
        "use std::fs as disk; fn f(){disk::OpenOptions::default();}",
        "type Disk=std::fs::OpenOptions; fn f(){<Disk>::default();}",
        "fn f(){let operation=std::fs::write;operation(p,b);}",
        "fn f(){let _=Default::default::<std::fs::OpenOptions>();}",
        "macro_rules! f {($root:ident,$module:ident,$kind:ident,$operation:ident)=>{$root::$module::$kind::$operation(p)}} fn g(){f!(std,fs,File,open)}",
        "macro_rules! f {($root:ident,$kind:ident)=>{$root::fs::$kind::create(p)}} fn g(){f!(std,File)}",
        "fn f(path: &std::path::Path){path.exists();}",
        "fn f(path: &std::path::Path){path.canonicalize();}",
        "fn f(file: &std::fs::File){file.sync_all();}",
        "unsafe extern \"C\" { #[link_name=\"open\"] fn innocent(); }",
        "unsafe extern \"C\" { fn open(); }",
        "unsafe extern \"system\" { #[link_name=concat!(\"Create\",\"FileW\")] fn innocent(); }",
        "fn f(){let native=b\"CreateFileW\";}",
        "fn f(){let native=c\"CreateFileW\";}",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/bypass.rs", source)?;
        assert!(
            check(repository.root(), &repository.architecture, &policy).is_err(),
            "accepted {source}"
        );
    }
    assert!(
        access::rust(
            "struct Data {metadata: String} fn f(value: Data){let _=value.metadata;}",
            &[]
        )?
        .is_empty()
    );
    assert!(access::rust("fn notify(){} fn f(){notify();}", &[])?.is_empty());
    assert!(!access::rust("use notify::Watcher; fn f(){}", &[])?.is_empty());
    Ok(())
}

/// Untracked, ignored, inactive and mixed-case sources are inspected outside registered subtrees.
#[test]
fn all_authored_roots_and_mixed_case_extensions_remain_governed() -> Result<()> {
    for path in
        ["native/new.CPP", "tools/new.Py", "installer/worker/new.pS1", "crates/host/src/new.Rs"]
    {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), ".gitignore", path)?;
        let body = if path.ends_with("Rs") {
            "#[cfg(unix)] fn f(){std::fs::read(p);}"
        } else {
            "CreateFileW(path)"
        };
        fixtures::write(repository.root(), path, body)?;
        assert!(
            check(repository.root(), &repository.architecture, &policy).is_err(),
            "accepted {path}"
        );
    }
    let (repository, mut policy) = fixture()?;
    policy.source_roots.pop();
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    Ok(())
}

/// Decoded identifiers, quoted properties, platform commands and alternate source languages reject.
#[test]
fn script_native_python_powershell_and_csharp_routes_require_reviews() -> Result<()> {
    for (extension, source) in [
        ("js", "dialog.open(); value.replace('a', 'b'); new File(parts, name);"),
        ("css", "dialog[open] { display: block; }"),
        ("cpp", "struct File {}; void replace();"),
    ] {
        assert!(
            access::script(source, extension).is_empty(),
            "misclassified {extension}: {source}"
        );
    }
    for (extension, source) in [
        ("mjs", r"const disk=require('node:\u0066s'); disk['write\u0046ileSync'](p,b)"),
        ("ts", r"const {readFile: harmless}=await import('node:fs/promises')"),
        ("py", "from pathlib import Path as Data; Data(p).write_bytes(b)"),
        ("py", "from tempfile import TemporaryDirectory as Scope; Scope()"),
        ("py", "entries = supplied_directory.iterdir()"),
        ("ps1", "gEt`-cOn`TeNt -LiteralPath $p"),
        ("ps1", "[System.IO.File]::WriteAllBytes($p,$bytes)"),
        ("ps1", "[system.io.file]::writealltext($p,$bytes)"),
        ("ps1", "[sYsTeM.iO.dIrEcToRy]::cReAtEdIrEcToRy($p)"),
        ("cs", "using Alias=System.IO.File; Alias.WriteAllText(path,data);"),
        ("cpp", "auto input=std::ifstream(path);"),
        ("patch", "+ import disk from 'node:fs'; disk.writeFile(path,data)"),
        ("cmake", "FILE(WRITE target bytes)"),
        ("gn", "data = read_file(path)"),
    ] {
        assert!(!access::script(source, extension).is_empty(), "missed {extension}: {source}");
        let (repository, policy) = fixture()?;
        fixtures::write(
            repository.root(),
            &format!("installer/worker/bypass.{extension}"),
            source,
        )?;
        assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    }
    let (repository, policy) = fixture()?;
    fixtures::write(
        repository.root(),
        "installer/worker/CMakeLists.txt",
        "FILE(WRITE target bytes)",
    )?;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    Ok(())
}

/// Public interfaces cannot leak library output methods or filesystem constructors through wrappers.
#[test]
fn raw_public_filesystem_capabilities_remain_restricted_through_aliases_and_traits() -> Result<()> {
    for source in [
        "pub use std::fs::File;",
        "mod entry {pub fn entry()->std::fs::File{loop{}}}pub use entry::entry;",
        "use std::fs::*; pub fn raw()->File{loop{}}",
        "type Hidden=std::fs::OpenOptions; pub type Escape=Hidden;",
        "mod nested{pub type Raw=std::fs::File;} pub use nested::Raw;",
        "pub struct Owner; impl AsRef<std::fs::File> for Owner{fn as_ref(&self)->&std::fs::File{loop{}}}",
        "pub struct Owner; impl std::ops::Deref for Owner{type Target=std::fs::OpenOptions;fn deref(&self)->&Self::Target{loop{}}}",
        "struct Source; trait Project{type Output;} impl Project for Source{type Output=std::fs::File;} pub type Escape=<Source as Project>::Output;",
        "pub fn render()->tiny_skia::Pixmap{loop{}}",
        "#[macro_export] macro_rules! expose {()=>{pub type Escape=std::fs::File;}}",
        "#[macro_export] macro_rules! expose {($root:ident,$kind:ident)=>{pub type Escape=$root::fs::$kind;}}",
    ] {
        let (repository, _) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/lib.rs", source)?;
        let found = crate::execution_exports::findings_with(
            &repository.root().join("crates/host"),
            &[crate::workspace::Dependency {
                name: "tiny-skia".into(),
                rename: None,
                kind: None,
                path: None,
            }],
            access::exported,
        )?;
        assert!(!found.is_empty(), "accepted interface {source}");
    }
    Ok(())
}

/// A source fingerprint refresh cannot silently widen a review's allowed capability identities.
#[test]
fn source_roles_and_exact_capability_lists_cannot_be_expanded_silently() -> Result<()> {
    for variant in 0..5 {
        let (repository, mut policy) = fixture()?;
        match variant {
            0 => policy.records[0].capabilities.push("std::fs::write".into()),
            1 => policy.records[0].capabilities.push("std::fs::rename".into()),
            2 => {
                policy.records[0].disposition = Disposition::Tooling;
            }
            3 => {
                policy.records[0].consumer = Some(Consumer::ExecutionMetadata);
            }
            _ => {
                policy.records[0].disposition = Disposition::Consumer;
                policy.records[0].consumer = Some(Consumer::ExecutionMetadata);
            }
        }
        assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    }
    let (repository, mut policy) = fixture()?;
    fixtures::write(
        repository.root(),
        &policy.records[0].path,
        "fn publish(){std::fs::rename(a,b);std::fs::write(c,d);}",
    )?;
    policy.records[0].source_sha256 =
        fingerprint(repository.root(), &[policy.records[0].path.clone()])?;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    Ok(())
}

/// Platform interop preserves explicit native handle loans without permitting filesystem objects.
#[test]
fn native_interop_review_cannot_authorize_public_file_or_builder_exports() -> Result<()> {
    let (repository, mut policy) = fixture()?;
    let path = "crates/host/src/native_interop.rs";
    fixtures::write(
        repository.root(),
        path,
        "pub fn observe(handle: std::os::windows::io::BorrowedHandle<'_>) {}",
    )?;
    let observed = super::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
        &policy.source_roots,
    )?;
    let source = observed
        .sources
        .into_iter()
        .find(|source| source.path == path)
        .ok_or_else(|| Failure::Policy("native interop source missing".into()))?;
    policy.records.push(Record {
        path: source.path,
        owner: source.owner,
        source_sha256: source.source_sha256,
        capabilities: source.capabilities.into_iter().collect(),
        disposition: Disposition::Boundary,
        boundary: Some(Boundary::NativeIpc),
        consumer: None,
        rationale: "Fixture borrows a caller-owned platform object; no file construction".into(),
        next_extraction: None,
        review_until: None,
        external_contract: None,
    });
    check(repository.root(), &repository.architecture, &policy)?;
    fixtures::write(repository.root(), path, "pub fn acquire()->std::fs::File{loop{}}")?;
    let changed = super::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
        &policy.source_roots,
    )?;
    let source = changed
        .sources
        .into_iter()
        .find(|source| source.path == path)
        .ok_or_else(|| Failure::Policy("raw file source missing".into()))?;
    policy.records[1].source_sha256 = source.source_sha256;
    policy.records[1].capabilities = source.capabilities.into_iter().collect();
    let failure = check(repository.root(), &repository.architecture, &policy)
        .err()
        .ok_or_else(|| Failure::Policy("native boundary exposed a File".into()))?;
    assert!(failure.to_string().contains("raw filesystem capability"));
    Ok(())
}

/// Tooling and typed-receiver roles have independent scope checks, even with fresh fingerprints.
#[test]
fn runtime_sources_cannot_claim_tooling_or_typed_metadata_permissions() -> Result<()> {
    for (role, consumer, expected) in [
        (Disposition::Tooling, None, "tooling review"),
        (Disposition::Consumer, Some(Consumer::ExecutionMetadata), "typed metadata consumer"),
    ] {
        let (repository, mut policy) = fixture()?;
        let path = "crates/host/src/direct.rs";
        fixtures::write(repository.root(), path, "fn read(){std::fs::read(path);}")?;
        policy.records.push(Record {
            path: path.into(),
            owner: "host".into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            capabilities: vec!["std::fs::read".into()],
            disposition: role,
            boundary: None,
            consumer,
            rationale: "Deliberately invalid source-role claim".into(),
            next_extraction: None,
            review_until: None,
            external_contract: None,
        });
        let failure = check(repository.root(), &repository.architecture, &policy)
            .err()
            .ok_or_else(|| Failure::Policy("runtime source escaped role validation".into()))?;
        assert!(failure.to_string().contains(expected), "wrong rejection: {failure}");
    }
    Ok(())
}

/// Native file-port consumers may use only their supported calls, never acquire another backend.
#[test]
fn native_injection_permissions_reject_rust_and_node_filesystem_construction() -> Result<()> {
    for (extension, body, allowed) in [
        ("rs", "fn read(){std::fs::read(path);}", false),
        ("mjs", "const disk=require('fs/promises');", false),
        ("ts", "const disk=process.getBuiltinModule(name);", false),
        ("ts", "nativeDownloads.readFile(name); nativeDownloads.writeFile(name, bytes);", true),
    ] {
        let (repository, mut policy) = fixture()?;
        let path = format!("installer/worker/consumer.{extension}");
        fixtures::write(repository.root(), &path, body)?;
        let actual = super::inventory(
            repository.root(),
            &repository.architecture,
            &repository.workspace,
            &policy.source_roots,
        )?;
        let source = actual
            .sources
            .into_iter()
            .find(|source| source.path == path)
            .ok_or_else(|| Failure::Policy("native injection fixture source missing".into()))?;
        policy.records.push(Record {
            path: source.path,
            owner: source.owner,
            source_sha256: source.source_sha256,
            capabilities: source.capabilities.into_iter().collect(),
            disposition: Disposition::Boundary,
            boundary: Some(Boundary::NativeInjection),
            consumer: None,
            rationale: "Exercise the exact native file-port permission boundary".into(),
            next_extraction: None,
            review_until: None,
            external_contract: None,
        });
        let result = check(repository.root(), &repository.architecture, &policy);
        if allowed {
            result?;
        } else {
            let failure = result.err().ok_or_else(|| {
                Failure::Policy("independent filesystem accepted as native injection".into())
            })?;
            assert!(failure.to_string().contains("native injection"), "wrong rejection: {failure}");
        }
    }
    Ok(())
}
