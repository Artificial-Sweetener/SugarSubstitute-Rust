// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive the reserved same-origin prefix and match only its exact routes.

use super::*;
use crate::bulk_channel_diagnostic::{Miss, Origin};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

const TOKEN: [u8; 16] = [0x5a; 16];
const PAGE: &str = "http://127.0.0.1:8188";

fn routes(endpoint: &Endpoint) -> (String, String) {
    let token = Token::new(TOKEN).hex();
    (format!("{}d/{token}", endpoint.prefix()), format!("{}u/{token}", endpoint.prefix()))
}

/// A Fetch API request exactly as the native adapter reports it.
fn observed<'a>(method: &'a str, uri: &'a str, origin: Header<'a>) -> Request<'a> {
    Request { fetch: true, method: Some(method), uri: Some(uri), origin }
}

#[test]
fn local_and_remote_roots_reserve_the_same_path_on_their_own_origin() -> TestResult {
    for (root, origin, path) in [
        ("http://127.0.0.1:8188/", "http://127.0.0.1:8188", "/__sugarsubstitute/bulk/"),
        ("http://localhost:8188/", "http://localhost:8188", "/__sugarsubstitute/bulk/"),
        ("http://[::1]:8188/", "http://[::1]:8188", "/__sugarsubstitute/bulk/"),
        ("https://comfy.example.net/", "https://comfy.example.net", "/__sugarsubstitute/bulk/"),
        ("https://comfy.example.net:443/", "https://comfy.example.net", "/__sugarsubstitute/bulk/"),
        ("http://10.0.0.7:80/", "http://10.0.0.7", "/__sugarsubstitute/bulk/"),
        (
            "https://Comfy.Example.NET:8443/",
            "https://comfy.example.net:8443",
            "/__sugarsubstitute/bulk/",
        ),
        ("https://bücher.example/", "https://xn--bcher-kva.example", "/__sugarsubstitute/bulk/"),
        ("https://proxy.example/comfy/", "https://proxy.example", "/comfy/__sugarsubstitute/bulk/"),
    ] {
        let endpoint = Endpoint::for_root(root)?;
        assert_eq!(endpoint.path(), path, "{root}");
        assert_eq!(endpoint.prefix(), format!("{origin}{path}"), "{root}");
        assert_eq!(endpoint.filter(), format!("{origin}{path}*"), "{root}");
        let (download, upload) = routes(&endpoint);
        let token = Token::new(TOKEN);
        let page = Header::Text(origin);
        assert_eq!(
            endpoint.route(observed("GET", &download, Header::Absent)),
            Ok(Route::Download(token))
        );
        assert_eq!(endpoint.route(observed("GET", &download, page)), Ok(Route::Download(token)));
        assert_eq!(endpoint.route(observed("POST", &upload, page)), Ok(Route::Upload(token)));
        assert_eq!(
            endpoint.route(observed("POST", &upload, Header::Absent)),
            Ok(Route::Upload(token))
        );
    }
    Ok(())
}

#[test]
fn roots_outside_a_plain_http_directory_are_refused() {
    let overlong = format!("https://proxy.example/{}/", "x".repeat(Endpoint::MAX_ROUTE_BYTES));
    for root in [
        "file:///C:/ComfyUI/",
        "ws://127.0.0.1:8188/",
        "data:text/html,x",
        "http://user:secret@127.0.0.1:8188/",
        "http://127.0.0.1:8188/?sc-panel-session=x",
        "http://127.0.0.1:8188/#x",
        "https://proxy.example/comfy",
        "not a url",
        overlong.as_str(),
    ] {
        assert!(Endpoint::for_root(root).is_err(), "{root}");
    }
}

#[test]
fn routes_match_only_their_own_scheme_host_port_and_base_path() -> TestResult {
    let local = Endpoint::for_root("http://127.0.0.1:8188/")?;
    let remote = Endpoint::for_root("https://proxy.example/comfy/")?;
    let token = Token::new(TOKEN).hex();
    for (endpoint, uri) in [
        (&local, format!("http://127.0.0.1:8189/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("https://127.0.0.1:8188/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("http://localhost:8188/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("http://127.0.0.2:8188/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("http://127.0.0.1:8188/base/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("http://127.0.0.1:8188/__sugarsubstitute/other/d/{token}")),
        (&local, format!("http://127.0.0.1:8188/api/__sugarsubstitute/bulk/d/{token}")),
        (&local, format!("https://sugarsubstitute-bulk.invalid/d/{token}")),
        (&remote, format!("https://proxy.example/__sugarsubstitute/bulk/d/{token}")),
        (&remote, format!("http://proxy.example/comfy/__sugarsubstitute/bulk/d/{token}")),
        (&remote, format!("https://proxy.example:8443/comfy/__sugarsubstitute/bulk/d/{token}")),
        (&remote, format!("https://other.example/comfy/__sugarsubstitute/bulk/d/{token}")),
    ] {
        assert_eq!(
            endpoint.route(observed("GET", &uri, Header::Absent)),
            Err(Miss::PrefixMismatch),
            "{uri}"
        );
    }
    Ok(())
}

#[test]
fn routes_require_exact_method_url_and_any_present_page_origin() -> TestResult {
    let endpoint = Endpoint::for_root(&format!("{PAGE}/"))?;
    let page = Header::Text(PAGE);
    let (download, upload) = routes(&endpoint);
    let token = Token::new(TOKEN).hex();
    let uppercase = download.to_uppercase();
    let uppercase_token = download.replace(&token, &token.to_uppercase());
    let other_route = format!("{}x/{token}", endpoint.prefix());
    let bare_route = format!("{}d/", endpoint.prefix());
    let doubled = download.replace("/bulk/", "/bulk//");
    let escaped = download.replace("/bulk/", "/%62ulk/");
    let suffixed: Vec<String> =
        ["?x=1", "#x", "/", "0"].iter().map(|suffix| format!("{download}{suffix}")).collect();
    let foreign = |uri, origin| (observed("GET", uri, origin), Miss::ForeignOrigin);
    let mut rejected = vec![
        (observed("POST", &download, page), Miss::RouteMismatch),
        (observed("GET", &upload, page), Miss::RouteMismatch),
        (observed("OPTIONS", &upload, page), Miss::RouteMismatch),
        (observed("HEAD", &download, Header::Absent), Miss::RouteMismatch),
        foreign(&download, Header::Text("http://127.0.0.1:8189")),
        foreign(&download, Header::Text("https://127.0.0.1:8188")),
        (observed("POST", &upload, Header::Text("http://localhost:8188")), Miss::ForeignOrigin),
        foreign(&download, Header::Text("null")),
        foreign(&download, Header::Text("")),
        foreign(&download, Header::Unreadable),
        (observed("POST", &upload, Header::Unreadable), Miss::ForeignOrigin),
        (observed("GET", &uppercase, page), Miss::PrefixMismatch),
        (observed("GET", &uppercase_token, page), Miss::MalformedCapability),
        (observed("GET", &other_route, page), Miss::RouteMismatch),
        (observed("GET", &bare_route, page), Miss::MalformedCapability),
        (observed("GET", endpoint.prefix(), page), Miss::RouteMismatch),
        (observed("GET", &doubled, page), Miss::RouteMismatch),
        (observed("GET", &escaped, page), Miss::PrefixMismatch),
        (Request { fetch: false, ..observed("GET", &download, page) }, Miss::NonFetchContext),
        (Request { fetch: false, ..observed("POST", &upload, page) }, Miss::NonFetchContext),
        (Request { method: None, ..observed("GET", &download, page) }, Miss::RouteMismatch),
        (Request { uri: None, ..observed("GET", &download, page) }, Miss::UrlUnreadable),
    ];
    rejected
        .extend(suffixed.iter().map(|uri| (observed("GET", uri, page), Miss::MalformedCapability)));
    for (request, miss) in rejected {
        assert_eq!(endpoint.route(request), Err(miss), "{request:?}");
    }
    Ok(())
}

#[test]
fn a_later_miss_proves_every_earlier_check_passed() -> TestResult {
    let endpoint = Endpoint::for_root(&format!("{PAGE}/"))?;
    let (download, _) = routes(&endpoint);
    let other = Header::Text("http://127.0.0.1:8189");
    let malformed = format!("{}d/{}", endpoint.prefix(), "z".repeat(32));
    fn hidden(request: Request<'_>) -> Request<'_> {
        Request { fetch: false, ..request }
    }
    // Each request fails several checks; the earliest in route order is reported.
    for (request, miss) in [
        (Request { uri: None, ..hidden(observed("POST", &download, other)) }, Miss::UrlUnreadable),
        (hidden(observed("GET", "http://x/", other)), Miss::PrefixMismatch),
        (hidden(observed("POST", &download, other)), Miss::RouteMismatch),
        (hidden(observed("GET", &malformed, other)), Miss::MalformedCapability),
        (hidden(observed("GET", &download, other)), Miss::ForeignOrigin),
        (hidden(observed("GET", &download, Header::Absent)), Miss::NonFetchContext),
    ] {
        assert_eq!(endpoint.route(request), Err(miss), "{request:?}");
    }
    Ok(())
}

#[test]
fn origin_headers_classify_against_the_page_origin() -> TestResult {
    let endpoint = Endpoint::for_root(&format!("{PAGE}/"))?;
    assert_eq!(endpoint.origin(Header::Absent), Origin::Absent);
    assert_eq!(endpoint.origin(Header::Text(PAGE)), Origin::Matching);
    assert_eq!(endpoint.origin(Header::Text("http://127.0.0.1:8189")), Origin::Other);
    assert_eq!(endpoint.origin(Header::Text("null")), Origin::Other);
    assert_eq!(endpoint.origin(Header::Unreadable), Origin::Unreadable);
    Ok(())
}
