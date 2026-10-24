// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adversarial network ownership, dependency identity, public API and source coverage fixtures.
use super::*;
use crate::{
    capability_source,
    fixtures::{self, Repository},
};
fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "adapter".into();
    repository.architecture.owners[0].concern =
        "Bounded native HTTP transport and checked handle custody".into();
    repository.architecture.owners[0].auxiliary_roots =
        vec!["tools".into(), "native".into(), "installer".into()];
    for root in ["tools", "native", "installer"] {
        fixtures::write(repository.root(), &format!("{root}/empty.py"), "pass")?;
    }
    let path = "crates/host/src/native.rs".to_owned();
    fixtures::write(repository.root(), &path, "fn f() { std::net::TcpStream::connect(a); }")?;
    let policy = Policy {
        schema_version: 1,
        owner: "host".into(),
        source_roots: ROOTS.iter().map(|s| (*s).into()).collect(),
        records: vec![Record {
            source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
            path,
            owner: "host".into(),
            boundary: Boundary::Implementation,
            rationale: "Sole transport boundary".into(),
        }],
        dependencies: Vec::new(),
        script_manifests: Vec::new(),
        external_boundaries: Vec::new(),
    };
    Ok((repository, policy))
}

/// A repository-wide rejection names every missing boundary for one complete review.
#[test]
fn missing_transport_reviews_report_every_source() -> Result<()> {
    let (repository, policy) = fixture()?;
    for path in ["tools/first.py", "native/second.py"] {
        fixtures::write(repository.root(), path, "from urllib.request import urlopen")?;
    }
    let failure =
        check(repository.root(), &repository.architecture, &repository.workspace, &policy)
            .err()
            .ok_or_else(|| Failure::Policy("Missing network routes were accepted".into()))?
            .to_string();
    assert!(failure.contains("tools/first.py"), "{failure}");
    assert!(failure.contains("native/second.py"), "{failure}");
    Ok(())
}

/// Standard-library servers and renamed module imports remain real transport boundaries.
#[test]
fn python_servers_and_module_aliases_require_exact_qualification_reviews() -> Result<()> {
    for source in [
        "from http.server import HTTPServer as Hidden",
        "from http import server as hidden",
        "import socketserver as hidden",
        "from socketserver import TCPServer as Hidden",
        "from http import client as hidden",
        "from urllib import request as hidden",
    ] {
        let (repository, mut policy) = fixture()?;
        let path = "tools/updater_feed_fixture.py".to_owned();
        fixtures::write(repository.root(), &path, source)?;
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err()
        );
        policy.records.push(Record {
            source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
            path,
            owner: "host".into(),
            boundary: Boundary::Fixture,
            rationale: "Explicit isolated HTTP qualification actor".into(),
        });
        check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
        let review = policy
            .records
            .last_mut()
            .ok_or_else(|| Failure::Policy("Fixture review absent".into()))?;
        review.path = "tools/unrelated.py".into();
        fixtures::write(repository.root(), &review.path, source)?;
        review.source_sha256 = fingerprint(repository.root(), std::slice::from_ref(&review.path))?;
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err()
        );
    }
    Ok(())
}

/// Real renamed Cargo bindings remain visible without a use statement at the callsite.
#[test]
fn cargo_bindings_retain_network_identity_in_qualified_source_access() -> Result<()> {
    let dependency = crate::workspace::Dependency {
        name: "reqwest".into(),
        rename: Some("innocent".into()),
        kind: Some("dev".into()),
        path: None,
    };
    let findings = source_access::rust("fn f() { innocent::Client::new(); }", &[dependency])?;
    assert!(findings.iter().any(|finding| finding.contains("reqwest")), "{findings:?}");
    Ok(())
}

/// Shared discovery preserves case and checks directories that only resemble installed dependencies.
#[test]
fn shared_source_discovery_cannot_hide_case_or_nested_dependency_named_directories() -> Result<()> {
    for (path, source) in [
        ("native/bypass.RS", "fn f() { std::net::TcpStream::connect(a); }"),
        ("native/bypass.CS", "var client = new HttpClient();"),
        ("tools/bypass.PS1", "[system.net.http.httpclient]::new()"),
        ("tools/bypass.ps1", "invoke-webrequest https://example.invalid/"),
        ("native/CMakeLists.txt", "file(DOWNLOAD https://example.invalid/source archive)"),
        ("crates/host/src/node_modules/hidden.js", r"globalThis['\x66etch'](url)"),
    ] {
        let (repository, policy) = fixture()?;
        check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
        fixtures::write(repository.root(), path, source)?;
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err(),
            "accepted {path}"
        );
    }
    Ok(())
}
#[test]
fn parsed_rust_rejects_construction_import_reexport_wrapper_macro_and_inactive_target_bypasses()
-> Result<()> {
    for source in [
        "use std::net::TcpStream as Hidden; fn f() { Hidden::connect(a); }",
        "pub use std::net as innocent; fn f() { innocent::TcpListener::bind(a); }",
        "use std::net::{self}; fn f() { net::UdpSocket::bind(a); }",
        "use std::net::*;",
        "type Hidden = std::net::TcpStream; fn wrapper() -> Hidden { f() }",
        "use windows::Win32::Networking::WinHttp::WinHttpOpen as hidden;",
        "#[cfg(unix)] fn f() { std::net::TcpStream::connect(a); }",
        "#[cfg(test)] mod tests { fn f() { reqwest::Client::new(); } }",
        "macro_rules! f { () => { std::net::TcpStream::connect(a) } }",
        "macro_rules! f { ($a:ident,$b:ident,$c:ident) => { $a::$b::$c() } } fn g() { f!(std,net,TcpStream); }",
        r#"unsafe extern "system" { #[link_name = "WinHttpOpen"] fn hidden(); }"#,
        "use curl as innocent; fn f() { innocent::easy::Easy::new(); }",
        "use tokio::net::TcpStream;",
        r#"fn f() { loader("WinHttpOpen"); }"#,
        r#"fn f() { loader(b"WinHttpOpen\0"); }"#,
        r#"fn f() { loader(c"WinHttpOpen"); }"#,
        r#"fn f() { let script = "await fetch(url)"; webview.execute_script(script); }"#,
        r#"macro_rules! f { () => { webview.execute_script("fetch(url)") } }"#,
        r#"fn f() { webview.execute_script("window['fetch'](url)"); }"#,
    ] {
        let (repository, policy) = fixture()?;
        check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
        fixtures::write(repository.root(), "crates/host/src/bypass.rs", source)?;
        // Outside the declared implementation package, the same syntax requires its own boundary.
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err(),
            "accepted {source}"
        );
    }
    Ok(())
}
#[test]
fn auxiliary_languages_and_roots_reject_direct_aliased_destructured_and_quoted_networking()
-> Result<()> {
    for extension in EXTENSIONS.iter().filter(|ext| **ext != "rs") {
        for source in [
            "import { request as hidden } from 'node:https'; hidden(a)",
            "const { fetch: hidden } = globalThis; hidden(a)",
            "window['fetch'](a)",
            "WinHttpOpen(a)",
            "from urllib.request import urlopen as hidden",
            "Invoke-WebRequest a",
            "import { connect as hidden } from 'node:tls'; hidden(a)",
            "const hidden = require('node:http2'); hidden.connect(a)",
            "[System.Net.Sockets.TcpClient]::new()",
        ] {
            for root in ["tools", "native", "installer"] {
                let (repository, policy) = fixture()?;
                fixtures::write(repository.root(), &format!("{root}/bypass.{extension}"), source)?;
                assert!(
                    check(
                        repository.root(),
                        &repository.architecture,
                        &repository.workspace,
                        &policy
                    )
                    .is_err(),
                    "accepted {root}/{extension}: {source}"
                );
            }
        }
    }
    for (extension, source) in [
        ("c", "int f() { return socket(2, 1, 0); }"),
        ("cpp", "auto hidden = &socket; auto stream = hidden(2, 1, 0);"),
        ("h", "int connect(int socket, const void *address, int length);"),
        ("mm", "auto hidden = &recv; hidden(stream, bytes, size, 0);"),
        ("py", "import socket as hidden; hidden.create_connection(address)"),
        ("py", "from socket import socket as hidden; hidden()"),
        ("py", "import http.client as hidden; hidden.HTTPSConnection(host)"),
        ("py", "import aiohttp as hidden; hidden.ClientSession()"),
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), &format!("native/bypass.{extension}"), source)?;
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err(),
            "accepted {extension}: {source}"
        );
    }
    Ok(())
}
#[test]
fn dependency_identity_rejects_renamed_dev_and_inactive_network_clients() -> Result<()> {
    for identity in
        ["reqwest", "ureq", "hyper", "socket2", "mio", "tokio", "curl", "tokio-tungstenite"]
    {
        for kind in [None, Some("dev"), Some("build")] {
            let (mut repository, policy) = fixture()?;
            repository.workspace.packages[0].dependencies.push(crate::workspace::Dependency {
                name: identity.into(),
                rename: Some("harmless".into()),
                kind: kind.map(str::to_owned),
                path: None,
            });
            assert!(
                check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                    .is_err()
            );
        }
    }
    Ok(())
}
#[test]
fn public_boundary_rejects_raw_network_types_aliases_macros_and_reexports() -> Result<()> {
    for source in [
        "pub use std::net::TcpStream as Innocent;",
        "use std::net::TcpStream as Hidden; pub fn escape() -> Hidden { f() }",
        "type Hidden = *mut std::ffi::c_void; pub struct Escape { pub raw: Hidden }",
        "type Hidden = *mut u8; pub fn escape() -> Hidden { f() }",
        "mod hidden { pub type Raw = std::net::TcpListener; } pub fn escape() -> hidden::Raw { f() }",
        "pub use windows::Win32::Networking::WinHttp::*;",
        "#[macro_export] macro_rules! escape { () => { std::net::TcpStream::connect(a) } }",
        r#"unsafe extern "system" { #[link_name = "WinHttpOpen"] pub fn harmless(); }"#,
    ] {
        let (repository, _) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/lib.rs", source)?;
        let found = execution_exports::findings_with(
            &repository.root().join("crates/host"),
            &[],
            access::exported,
        )?;
        assert!(!found.is_empty(), "accepted {source}");
    }
    Ok(())
}
#[test]
fn stale_boundaries_unknown_schemas_owners_and_missing_coverage_reject() -> Result<()> {
    for variant in 0..12 {
        let (mut repository, mut policy) = fixture()?;
        check(repository.root(), &repository.architecture, &repository.workspace, &policy)?;
        match variant {
            0 => policy.schema_version = 2,
            1 => policy.owner = "unknown".into(),
            2 => policy.source_roots.pop().map(|_| ()).unwrap_or(()),
            3 => policy.records[0].source_sha256 = "stale".into(),
            4 => policy.records[0].owner = "foreign".into(),
            5 => policy.records[0].boundary = Boundary::Upstream,
            6 => policy.records[0].rationale.clear(),
            7 => {
                let mut duplicate = fixture()?.1;
                policy.records.push(duplicate.records.remove(0));
            }
            8 => policy.records[0].path = "../outside.rs".into(),
            9 => {
                fixtures::write(repository.root(), &policy.records[0].path, "fn ordinary() {}")?;
                policy.records[0].source_sha256 =
                    fingerprint(repository.root(), &[policy.records[0].path.clone()])?;
            }
            10 => policy.dependencies.push(Dependency {
                package: "host".into(),
                identity: "reqwest".into(),
                rationale: "Unused".into(),
            }),
            _ => {
                repository.architecture.owners[0].auxiliary_roots.push("uncovered".into());
                fixtures::write(repository.root(), "uncovered/main.js", "fetch(url)")?;
            }
        }
        assert!(
            check(repository.root(), &repository.architecture, &repository.workspace, &policy)
                .is_err()
        );
    }
    for source in [
        "schema_version = 1\nunknown = true",
        "schema_version=1\nowner='host'\nrecords=[]\ndependencies=[]",
    ] {
        assert!(toml::from_str::<Policy>(source).is_err());
    }
    Ok(())
}

#[test]
fn browser_upstream_ipc_and_inspection_labels_cannot_authorize_native_http_construction()
-> Result<()> {
    let (_, mut policy) = fixture()?;
    let raw = capability_source::findings(
        "use windows::Win32::Networking::WinHttp::WinHttpOpen as hidden; fn f() { hidden(); }",
        access::path,
        access::fragments,
        access::ordinary,
    )?;
    assert!(!raw.is_empty());
    for boundary in
        [Boundary::Browser, Boundary::Upstream, Boundary::ExecutionIpc, Boundary::Inspection]
    {
        let record = &mut policy.records[0];
        record.boundary = boundary;
        record.path = "installer/worker/provision.ts".into();
        assert!(!category_admits(record, &raw));
    }
    policy.records[0].boundary = Boundary::ExecutionIpc;
    let socket = capability_source::findings(
        "use tokio::net::{TcpListener,TcpStream}; use std::{net::Shutdown};",
        access::path,
        access::fragments,
        access::ordinary,
    )?;
    assert!(category_admits(&policy.records[0], &socket));
    policy.records[0].boundary = Boundary::Upstream;
    policy.records[0].path = "installer/worker/downloader.ts".into();
    assert!(category_admits(&policy.records[0], &["@upstream/lib/download".into()]));
    assert!(!category_admits(&policy.records[0], &["@upstream/lib/fetch".into()]));
    assert!(!category_admits(&policy.records[0], &raw));
    Ok(())
}
