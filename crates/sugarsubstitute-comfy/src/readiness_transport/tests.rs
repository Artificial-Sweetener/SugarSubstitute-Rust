// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject hostile HTTP framing at the complete-header boundary without live networking.
use super::{headers::parse_headers, probe::Endpoint};
mod verification;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn backend_script_binding_accepts_only_exact_dos_spelling_with_optional_verbatim_prefix() {
    use std::path::Path;
    let canonical = Path::new(r"\\?\E:\comfyui\main.py");
    for launch in [r"E:\comfyui\main.py", r"\\?\E:\comfyui\main.py"] {
        assert!(super::types::backend_script_matches(canonical, Path::new(launch)));
    }
    for launch in [
        r"E:\other\main.py",
        r"e:\comfyui\main.py",
        r"E:\ComfyUI\main.py",
        r"E:\comfyui\.\main.py",
        r"E:\comfyui\..\comfyui\main.py",
        r"E:/comfyui/main.py",
        r"\\server\share\comfyui\main.py",
        r"\\?\UNC\server\share\comfyui\main.py",
        r"\\.\E:\comfyui\main.py",
    ] {
        assert!(!super::types::backend_script_matches(canonical, Path::new(launch)));
    }
}
#[test]
fn bounded_json_headers_accept_exact_length_or_chunked_framing() -> TestResult {
    assert_eq!(
        parse_headers(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: 42\r\n\r\n",
            Endpoint::Stats
        )?,
        Some(42)
    );
    assert_eq!(
        parse_headers(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
            Endpoint::Stats
        )?,
        None
    );
    Ok(())
}
#[test]
fn duplicate_or_ambiguous_length_and_transfer_headers_are_rejected() {
    for extra in [
        "Content-Length: 1\r\nContent-Length: 2",
        "Content-Length: 1\r\nTransfer-Encoding: chunked",
        "Transfer-Encoding: gzip, chunked",
        "Content-Length: -1",
        "Content-Length: 65537",
    ] {
        assert!(
            parse_headers(
                &format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra}\r\n\r\n"),
                Endpoint::Stats
            )
            .is_err()
        );
    }
}
#[test]
fn unexpected_encoding_and_non_utf8_json_content_type_are_rejected() {
    for extra in [
        "Content-Encoding: gzip",
        "Content-Type: text/html",
        "Content-Type: application/json; charset=latin1",
        "Content-Type: application/json\r\nContent-Type: application/json",
    ] {
        let initial = if extra.starts_with("Content-Type") {
            ""
        } else {
            "Content-Type: application/json\r\n"
        };
        assert!(
            parse_headers(&format!("HTTP/1.1 200 OK\r\n{initial}{extra}\r\n\r\n"), Endpoint::Stats)
                .is_err()
        );
    }
}

#[test]
fn asset_response_length_uses_the_pinned_source_bound_independently_of_json_limits() -> TestResult {
    let maximum = crate::readiness::MAX_SYSTEM_STATS_BYTES as u64 + 1;
    let endpoint = Endpoint::Asset(super::probe::ExpectedAsset {
        path: "/assets/served.bin",
        bytes: maximum,
        hash: [0; 32],
    });
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {maximum}\r\n\r\n"
    );
    assert_eq!(parse_headers(&headers, endpoint)?, Some(maximum));
    assert!(
        parse_headers(
            &headers.replace("application/octet-stream", "application/json"),
            Endpoint::Stats
        )
        .is_err()
    );
    assert!(
        parse_headers(&headers.replace(&maximum.to_string(), &(maximum + 1).to_string()), endpoint)
            .is_err()
    );
    Ok(())
}
#[test]
fn binary_asset_responses_reject_decompression_and_ambiguous_framing_before_body_consumption() {
    let endpoint = Endpoint::Asset(super::probe::ExpectedAsset {
        path: "/assets/served.bin",
        bytes: 4,
        hash: [0; 32],
    });
    for fields in [
        "Content-Encoding: gzip\r\nContent-Length: 4",
        "Content-Length: 4\r\nTransfer-Encoding: chunked",
        "Content-Length: 4\r\nContent-Length: 4",
        "Transfer-Encoding: gzip, chunked",
    ] {
        assert!(parse_headers(&format!("HTTP/1.1 200 OK\r\n{fields}\r\n\r\n"), endpoint).is_err());
    }
}
