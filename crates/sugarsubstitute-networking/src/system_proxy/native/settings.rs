// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate OS configuration and ordered proxy results without interpreting PAC scripts.
use super::Route;
#[cfg(test)]
mod tests;
use std::io;
use windows::{
    Win32::{
        Foundation::{GlobalFree, HGLOBAL},
        Networking::WinHttp::*,
    },
    core::PCWSTR,
};

/// Read current user/network settings; Windows owns static fallback and bypass interpretation.
pub(super) fn options(
    pac: &mut Vec<u16>,
    explicit: Option<&str>,
) -> io::Result<WINHTTP_AUTOPROXY_OPTIONS> {
    let mut options = WINHTTP_AUTOPROXY_OPTIONS::default();
    if let Some(address) = explicit {
        // Isolated native qualification supplies a PAC URL without changing user settings.
        pac.extend(address.encode_utf16().chain(Some(0)));
    } else {
        let mut config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
        // SAFETY: OS fills its documented structure; every allocated string is freed below.
        unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut config) }.map_err(io::Error::other)?;
        options.dwFlags = WINHTTP_AUTOPROXY_ALLOW_STATIC | WINHTTP_AUTOPROXY_ALLOW_CM;
        if config.fAutoDetect.as_bool() {
            options.dwFlags |= WINHTTP_AUTOPROXY_AUTO_DETECT;
            options.dwAutoDetectFlags =
                WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A;
        }
        // SAFETY: strings are owned null-terminated allocations returned by WinHTTP.
        let copied = unsafe { wide(config.lpszAutoConfigUrl.0, 8192) };
        let mut failure = None;
        for pointer in [config.lpszAutoConfigUrl.0, config.lpszProxy.0, config.lpszProxyBypass.0] {
            if !pointer.is_null() {
                // SAFETY: these allocations are explicitly documented as GlobalFree-owned.
                if let Err(cause) = unsafe { GlobalFree(Some(HGLOBAL(pointer.cast()))) } {
                    failure = Some(io::Error::other(cause));
                }
            }
        }
        if let Some(cause) = failure {
            return Err(cause);
        }
        if let Some(address) = copied?.filter(|value| !value.is_empty()) {
            pac.extend(address.encode_utf16().chain(Some(0)));
        }
    }
    if !pac.is_empty() {
        options.dwFlags |= WINHTTP_AUTOPROXY_CONFIG_URL;
        options.lpszAutoConfigUrl = PCWSTR(pac.as_ptr());
    }
    Ok(options)
}

/// Copy only supported, bounded routes before the OS result allocation is freed.
/// # Safety
/// `result` and its entry strings must be live WinHTTP result allocations.
pub(super) unsafe fn routes(result: &WINHTTP_PROXY_RESULT) -> io::Result<Vec<Route>> {
    if result.cEntries > 16 || (result.cEntries != 0 && result.pEntries.is_null()) {
        return Err(invalid("Windows proxy result exceeds its bound"));
    }
    if result.cEntries == 0 {
        return Ok(vec![Route::Direct]);
    }
    // SAFETY: WinHttpGetProxyResult owns this bounded entry array until WinHttpFreeProxyResult.
    let entries = unsafe { std::slice::from_raw_parts(result.pEntries, result.cEntries as usize) };
    entries
        .iter()
        .map(|entry| {
            if !entry.fProxy.as_bool() || entry.fBypass.as_bool() {
                return Ok(Route::Direct);
            }
            let scheme = match entry.ProxyScheme {
                WINHTTP_INTERNET_SCHEME_HTTP => "http",
                WINHTTP_INTERNET_SCHEME_HTTPS => "https",
                _ => return Err(invalid("Windows selected an unsupported proxy scheme")),
            };
            // SAFETY: a proxy result entry owns a terminated host string until the result is freed.
            let host = unsafe { wide(entry.pwszProxy.0, 1024) }?
                .ok_or_else(|| invalid("Windows proxy host is absent"))?;
            if entry.ProxyPort == 0
                || host.chars().any(|value| {
                    value.is_whitespace() || matches!(value, '/' | '\\' | '@' | '?' | '#')
                })
            {
                return Err(invalid("Windows proxy address is invalid"));
            }
            let host = if host.contains(':') && !host.starts_with('[') {
                format!("[{host}]")
            } else {
                host
            };
            let url = url::Url::parse(&format!("{scheme}://{host}:{}", entry.ProxyPort))
                .map_err(io::Error::other)?;
            Ok(Route::Proxy { url: url.to_string() })
        })
        .collect()
}
/// Copy a terminated OS string while enforcing the protocol's size limit.
unsafe fn wide(pointer: *const u16, maximum: usize) -> io::Result<Option<String>> {
    if pointer.is_null() {
        return Ok(None);
    }
    for length in 0..=maximum {
        // SAFETY: caller supplies an OS-owned terminated string; inspect only through its NUL.
        if unsafe { *pointer.add(length) } == 0 {
            // SAFETY: the preceding scan established this exact initialized string length.
            return String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, length) })
                .map(Some)
                .map_err(io::Error::other);
        }
    }
    Err(invalid("Windows proxy string exceeds its bound"))
}
/// Report a policy/shape violation without including raw configuration contents.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
