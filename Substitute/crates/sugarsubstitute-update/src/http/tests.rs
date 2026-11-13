// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Destination and framing rejection precedes native I/O and cache mutation.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn production_policy_only_accepts_explicit_https_origins_and_flat_assets() -> TestResult {
    for base in [
        "http://example.test/",
        "https://user@example.test/",
        "https://example.test/?x=1",
        "https://example.test/#x",
        " https://example.test/",
        "https://example.test/feed",
    ] {
        assert!(Policy::https(base, &[]).is_err(), "{base}");
    }
    let policy = Policy::https("https://example.test/releases/", &[])?;
    for name in ["", ".", "..", "../root.json", "a/b", "a\\b", "x?secret", "x#fragment", "%2f"] {
        assert!(policy.asset(name).is_err(), "{name}");
    }
    let asset = policy.asset("1.snapshot.json")?;
    assert_eq!(asset.host()?, "example.test");
    assert_eq!(asset.port()?, 443);
    assert_eq!(asset.request_target(), "/releases/1.snapshot.json");
    assert!(asset.secure());
    assert!(Policy::https("https://other.test/", &[])?.admits(&asset).is_err());
    Ok(())
}

#[test]
fn redirects_preserve_queries_but_never_expand_configured_authority() -> TestResult {
    let policy = Policy::https("https://example.test/releases/", &["https://cdn.test:8443"])?;
    let first = policy.asset("package")?;
    let next = policy.redirect(&first, "https://cdn.test:8443/object?signature=secret")?;
    assert_eq!(next.request_target(), "/object?signature=secret");
    assert_eq!(next.port()?, 8443);
    assert!(policy.admits(&next).is_ok());
    for location in [
        "http://example.test/file",
        "https://cdn.test/file",
        "https://evil.test/file",
        "https://example.test.evil.test/file",
        "https://user:pass@example.test/file",
        "/file#fragment",
        "/file\r\nx:y",
    ] {
        assert!(policy.redirect(&first, location).is_err(), "{location}");
    }
    let local = Policy::loopback(31234)?;
    let endpoint = local.asset("timestamp.json")?;
    assert!(!endpoint.secure());
    assert!(local.redirect(&endpoint, "http://127.0.0.1:31235/x").is_err());
    assert!(local.redirect(&endpoint, "http://localhost:31234/x").is_err());
    assert!(Policy::loopback(0).is_err());
    Ok(())
}

#[test]
fn ambiguous_and_encoded_framing_is_rejected_before_reading_any_body() {
    for fields in [
        "Content-Length: 4\r\nContent-Length: 4",
        "Content-Length: 4,4",
        "Content-Length: +4",
        "Content-Length: 4\r\nTransfer-Encoding: chunked",
        "Transfer-Encoding: gzip, chunked",
        "Content-Range: bytes 0-3/4\r\nContent-Range: bytes 0-3/4",
        "Content-Encoding: gzip",
        "Location: /a\r\nLocation: /b",
        " Content-Length: 4",
        "Bad Name: value",
        "X: a\nb",
    ] {
        assert!(
            Headers::parse(200, &format!("HTTP/1.1 200 OK\r\n{fields}\r\n\r\n")).is_err(),
            "{fields}"
        );
    }
    assert!(Headers::parse(200, "HTTP/1.1 206 OK\r\n\r\n").is_err());
    assert!(Headers::parse(200, "HTTP/1.1 200 OK\r\n\r\nX: a").is_err());
    assert!(Headers::parse(200, &format!("HTTP/1.1 200 OK\r\nX: {}", "a".repeat(16384))).is_err());
}

#[test]
fn valid_metadata_and_package_fields_keep_separate_response_rules() -> TestResult {
    for status in [401, 404, 429, 503] {
        assert_eq!(
            Headers::parse(status, &format!("HTTP/1.1 {status} Error\r\n\r\n"))?.metadata(4),
            Err(Rejection::Status(status))
        );
    }
    let metadata = Headers::parse(200, "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")?;
    metadata.metadata(4)?;
    assert!(metadata.metadata(3).is_err());
    let chunked = Headers::parse(200, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")?;
    chunked.metadata(4)?;
    assert_eq!(chunked.length(), None);
    let package = Headers::parse(
        206,
        "HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Range: bytes 4-7/8\r\n\r\n",
    )?;
    assert!(package.metadata(8).is_err());
    assert_eq!(package.package_response().range, Some("bytes 4-7/8"));
    Ok(())
}

#[test]
fn exact_endpoints_stay_on_the_configured_origins() -> TestResult {
    let policy = Policy::https("https://api.example.test/", &["https://cdn.example.test/"])?;
    let api = policy.endpoint("https://api.example.test/nodes/Pack/install?version=1.2.0")?;
    assert_eq!(api.request_target(), "/nodes/Pack/install?version=1.2.0");
    assert_eq!(policy.endpoint("https://cdn.example.test/a/node.zip")?.host()?, "cdn.example.test");
    for url in [
        "http://api.example.test/nodes",
        "https://evil.test/nodes",
        "https://api.example.test:8443/nodes",
        "https://user@api.example.test/nodes",
        "https://api.example.test/nodes#x",
        "/nodes",
        " https://api.example.test/nodes",
    ] {
        assert!(policy.endpoint(url).is_err(), "{url}");
    }
    Ok(())
}
