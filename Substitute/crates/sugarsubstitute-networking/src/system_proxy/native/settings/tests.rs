// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unsupported/oversized OS results and preserve bypass and IPv6 decisions.
use super::*;
use windows::core::PWSTR;

#[test]
fn route_translation_preserves_bypass_and_rejects_unsupported_policy() -> io::Result<()> {
    let mut name: Vec<u16> = "::1".encode_utf16().chain(Some(0)).collect();
    assert_eq!(
        translate(&mut name, false, WINHTTP_INTERNET_SCHEME_HTTP)?,
        vec![Route::Proxy { url: "http://[::1]:8080/".into() }]
    );
    assert_eq!(translate(&mut name, true, WINHTTP_INTERNET_SCHEME_HTTP)?, vec![Route::Direct]);
    assert!(translate(&mut name, false, WINHTTP_INTERNET_SCHEME(999)).is_err());
    let oversized = WINHTTP_PROXY_RESULT { cEntries: 17, pEntries: std::ptr::null_mut() };
    // SAFETY: invalid count is rejected before any array access.
    assert!(unsafe { routes(&oversized) }.is_err());
    Ok(())
}

/// Build one live OS-shaped result around the fixture's terminated host string.
fn translate(
    name: &mut [u16],
    bypass: bool,
    scheme: WINHTTP_INTERNET_SCHEME,
) -> io::Result<Vec<Route>> {
    let mut entry = WINHTTP_PROXY_RESULT_ENTRY {
        fProxy: true.into(),
        fBypass: bypass.into(),
        ProxyScheme: scheme,
        pwszProxy: PWSTR(name.as_mut_ptr()),
        ProxyPort: 8080,
    };
    let result = WINHTTP_PROXY_RESULT { cEntries: 1, pEntries: &raw mut entry };
    // SAFETY: the single entry and terminated host string remain live throughout translation.
    unsafe { routes(&result) }
}
