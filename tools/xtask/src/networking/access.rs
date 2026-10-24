// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Conservative network capability identities across parsed Rust and auxiliary source tokens.
use super::scripts;
use std::collections::BTreeSet;
/// Dependencies are checked by Cargo identity, including aliases and inactive target/dev edges.
pub(super) fn dependency(name: &str) -> bool {
    matches!(
        name.replace('-', "_").as_str(),
        "reqwest"
            | "ureq"
            | "hyper"
            | "hyper_util"
            | "curl"
            | "curl_sys"
            | "isahc"
            | "surf"
            | "attohttpc"
            | "minreq"
            | "awc"
            | "socket2"
            | "mio"
            | "quinn"
            | "s2n_quic"
            | "tungstenite"
            | "tokio_tungstenite"
            | "async_net"
            | "async_std"
            | "smol"
            | "tokio"
    )
}
/// Restricted native symbols include construction, configuration, transfer and closure.
fn native(name: &str) -> bool {
    name.starts_with("WinHttp")
        || name.starts_with("InternetOpen")
        || name.starts_with("InternetConnect")
        || name.starts_with("HttpSendRequest")
        || name.starts_with("HttpOpenRequest")
        || name.starts_with("InternetRead")
        || name.starts_with("InternetClose")
        || name.starts_with("CFHTTP")
        || name.starts_with("CFReadStreamCreateForHTTP")
        || name.starts_with("curl_")
        || matches!(
            name,
            "WSASocketA"
                | "WSASocketW"
                | "WSAConnect"
                | "WSASend"
                | "WSARecv"
                | "socket"
                | "connect"
                | "send"
                | "recv"
                | "closesocket"
                | "getaddrinfo"
        )
}
/// Pure socket-address types and process-table inspection do not grant transport effects.
pub(super) fn path(parts: &[String]) -> bool {
    parts.first().is_some_and(|name| dependency(name) && name != "tokio")
        || parts.windows(2).any(|p| p[0] == "std" && p[1] == "net")
        || parts.windows(2).any(|p| p[0] == "tokio" && p[1] == "net")
        || parts.iter().any(|name| name == "WinHttp" || name == "WinInet" || name == "HINTERNET")
        || parts.last().is_some_and(|name| native(name))
}
/// Bare ordinary send/connect methods need namespace resolution; native prefixed names do not.
pub(super) fn ordinary(name: &str) -> bool {
    matches!(name, "send" | "connect" | "recv" | "socket")
}
/// Macro path fragments must include an actual network namespace, not unrelated field names.
pub(super) fn fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| {
        (native(name) && !ordinary(name))
            || dependency(name) && name != "tokio"
            || matches!(name.as_str(), "WinHttp" | "WinInet" | "HINTERNET")
    }) || names.contains("net") && (names.contains("std") || names.contains("tokio"))
}
/// Script/native lexical inspection catches aliases, destructuring and quoted properties.
/// Arbitrary computed names and dependency-internal behavior still require review/qualification.
pub(super) fn lexical(source: &str) -> Vec<String> {
    let decoded = format!("{source}\n{}", crate::source_tokens::tokens(source).join(" "));
    let source = decoded.as_str();
    let names: BTreeSet<_> =
        source.split(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '-')).collect();
    let mut found: BTreeSet<String> = names
        .iter()
        .filter(|name| {
            native(name) && !ordinary(name)
                || matches!(
                    **name,
                    "fetch"
                        | "XMLHttpRequest"
                        | "WebSocket"
                        | "EventSource"
                        | "createConnection"
                        | "createServer"
                        | "downloadArtifact"
                        | "HttpClient"
                        | "WebClient"
                        | "TcpClient"
                        | "UdpClient"
                        | "TcpListener"
                        | "Invoke-WebRequest"
                        | "Invoke-RestMethod"
                        | "urlopen"
                        | "urlretrieve"
                        | "curl"
                        | "wget"
                )
        })
        .map(|name| (*name).to_owned())
        .collect();
    for namespace in [
        "node:net",
        "node:http",
        "node:https",
        "node:dgram",
        "node:tls",
        "node:http2",
        "electron",
        "urllib.request",
        "@upstream/lib/download",
        "@upstream/lib/fetch",
    ] {
        if source.contains(namespace) && (namespace != "electron" || names.contains("net")) {
            found.insert(namespace.into());
        }
    }
    for client in scripts::CLIENTS.iter().filter(|client| **client != "electron") {
        if source.contains(&format!("'{client}'")) || source.contains(&format!("\"{client}\"")) {
            found.insert((*client).into());
        }
    }
    if source.contains("import requests")
        || source.contains("from requests")
        || source.contains("requests.")
    {
        found.insert("requests".into());
    }
    found.into_iter().collect()
}
/// Language-specific primitives cannot rely on Rust namespaces or JavaScript module identities.
/// Conservative identifiers also retain aliased C function pointers and Python module imports.
pub(super) fn auxiliary(source: &str, extension: &str) -> Vec<String> {
    let mut found: BTreeSet<_> = lexical(source).into_iter().collect();
    let decoded = format!("{source}\n{}", crate::source_tokens::tokens(source).join(" "));
    let source = decoded.as_str();
    let names: BTreeSet<_> = source.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
    if matches!(extension, "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" | "hxx" | "m" | "mm") {
        for name in names.iter().filter(|name| native(name) && ordinary(name)) {
            found.insert((*name).to_owned());
        }
    }
    if extension == "py" {
        for name in [
            "socket",
            "aiohttp",
            "httplib",
            "socketserver",
            "HTTPServer",
            "ThreadingHTTPServer",
            "BaseHTTPRequestHandler",
            "TCPServer",
            "UDPServer",
            "ThreadingTCPServer",
            "ThreadingUDPServer",
        ] {
            if names.contains(name) {
                found.insert(name.to_owned());
            }
        }
        for member in ["client", "server"] {
            if source.contains(&format!("http.{member}"))
                || names.contains("http") && names.contains(member)
            {
                found.insert(format!("http.{member}"));
            }
        }
        if source.contains("urllib.request")
            || names.contains("urllib") && names.contains("request")
        {
            found.insert("urllib.request".into());
        }
    }
    if extension == "ps1" {
        for name in [
            "HttpClient",
            "WebClient",
            "TcpClient",
            "UdpClient",
            "TcpListener",
            "Invoke-WebRequest",
            "Invoke-RestMethod",
            "curl",
            "wget",
        ] {
            if source
                .split(|character: char| {
                    !character.is_alphanumeric() && !matches!(character, '_' | '-')
                })
                .any(|word| word.eq_ignore_ascii_case(name))
            {
                found.insert(name.into());
            }
        }
    }
    if extension == "cmake" {
        let words: BTreeSet<_> = names.iter().map(|name| name.to_ascii_lowercase()).collect();
        if words.contains("file") && (words.contains("download") || words.contains("upload"))
            || words
                .iter()
                .any(|word| matches!(word.as_str(), "fetchcontent_declare" | "externalproject_add"))
        {
            found.insert("cmake_network".into());
        }
    }
    found.into_iter().collect()
}
/// Raw pointer aliases are also forbidden public capabilities in the transport API.
pub(super) fn exported(parts: &[String]) -> bool {
    path(parts)
        || parts.iter().any(|name| {
            matches!(name.as_str(), "<raw-pointer>" | "c_void" | "HANDLE" | "HINTERNET")
        })
}

/// Decoded literals can contain actual embedded JavaScript or statically named native exports.
/// Computed names and data-flow through dynamically loaded code still require source review.
pub(super) fn literal(value: &str) -> bool {
    let symbol = value.trim_end_matches('\0');
    if native(symbol)
        && !ordinary(symbol)
        && symbol != "WinHttp"
        && symbol != "WinInet"
        && symbol.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return true;
    }
    let normalized: String = value.chars().filter(|character| !character.is_whitespace()).collect();
    [
        "fetch",
        "XMLHttpRequest",
        "WebSocket",
        "EventSource",
        "net.request",
        "net.fetch",
        "http.request",
        "https.request",
    ]
    .iter()
    .any(|name| normalized.contains(&format!("{name}(")))
        || ["node:net", "node:http", "node:https", "node:dgram", "node:tls", "node:http2"]
            .iter()
            .any(|name| value.contains(name))
        || ["fetch", "request"].iter().any(|name| {
            normalized.contains(&format!("['{name}']("))
                || normalized.contains(&format!("[\"{name}\"]("))
        })
}
