// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve the host-owned bulk origin to the current panel document.
//!
//! WebView2 raises `WebResourceRequested` on this UI apartment for every fetch to the reserved
//! origin. The handler answers synchronously: native getters return before the transfer
//! registry is borrowed, a request body is read in claimed bounded steps, and every unmatched
//! request receives an empty 404 while no body is retained.

mod response;

use super::*;
use crate::main_bulk::{Refusal, Route, Transfers, UploadBuffer};
use std::{ffi::c_void, time::Instant};
use webview2_com::WebResourceRequestedEventHandler;
use windows::{
    Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    core::{HRESULT, w},
};

/// Longest URL, method or `Origin` header the bulk origin can match, in UTF-16 units.
const MAX_TEXT_UNITS: usize = 128;

/// Generate one 128-bit token from the system-preferred cryptographic generator.
pub(crate) fn random_token() -> Result<[u8; 16], DesktopError> {
    let mut bytes = [0; 16];
    // SAFETY: The system-preferred provider takes no algorithm handle and fills this owned
    // writable buffer of exactly its slice length before returning.
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|cause| DesktopError::new("generate_bulk_token", cause))?;
    Ok(bytes)
}

/// Retain the bulk-origin handler and filter registrations through checked removal.
pub(super) struct BulkOrigin {
    transfers: Rc<Transfers>,
    handler: Option<i64>,
    filtered: bool,
    uncertain: bool,
}

impl BulkOrigin {
    pub(super) fn new(transfers: Rc<Transfers>) -> Self {
        Self { transfers, handler: None, filtered: false, uncertain: false }
    }

    /// Install the handler before the filter, one native registration per call.
    pub(super) fn install(
        &mut self,
        core: &ICoreWebView2,
        events: &Rc<Events>,
    ) -> Result<bool, DesktopError> {
        if self.uncertain {
            return Err(DesktopError::message("install_bulk_origin", "registration is uncertain"));
        }
        if self.handler.is_none() {
            let environment = environment(core)?;
            let handler = handler(events.clone(), self.transfers.clone(), environment);
            let mut token = 0;
            // SAFETY: The retained core and handler belong to this UI apartment; the token is
            // adopted before the status so a partial registration still retires.
            let result = unsafe { core.add_WebResourceRequested(&handler, &mut token) };
            if result.is_ok() || token != 0 {
                self.handler = Some(token);
            } else {
                self.uncertain = true;
            }
            result.map_err(|cause| DesktopError::new("install_bulk_origin", cause))?;
            return Ok(false);
        }
        if !self.filtered {
            let filter = HSTRING::from(crate::main_bulk::FILTER);
            // SAFETY: The bounded filter string outlives this synchronous call on the UI thread.
            let result = unsafe {
                core.AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH)
            };
            // A failed add cannot prove the filter absent, so removal is still attempted.
            self.filtered = true;
            result.map_err(|cause| DesktopError::new("install_bulk_filter", cause))?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Remove the filter before the handler, one native removal per call.
    pub(super) fn retire_step(&mut self, core: &ICoreWebView2) -> Result<bool, DesktopError> {
        self.transfers.close();
        if self.uncertain {
            return Ok(false);
        }
        if self.filtered {
            let filter = HSTRING::from(crate::main_bulk::FILTER);
            // SAFETY: The exact filter registered above is removed on its creating UI thread.
            unsafe {
                core.RemoveWebResourceRequestedFilter(
                    &filter,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH,
                )
            }
            .map_err(|cause| DesktopError::new("remove_bulk_filter", cause))?;
            self.filtered = false;
            return Ok(false);
        }
        if let Some(token) = self.handler {
            // SAFETY: This exact token belongs to the retained core on its UI thread.
            unsafe { core.remove_WebResourceRequested(token) }
                .map_err(|cause| DesktopError::new("remove_bulk_origin", cause))?;
            self.handler = None;
            return Ok(false);
        }
        Ok(true)
    }
}

/// Retain the environment that constructs every bulk response.
fn environment(core: &ICoreWebView2) -> Result<ICoreWebView2Environment, DesktopError> {
    let core: ICoreWebView2_2 =
        core.cast().map_err(|cause| DesktopError::new("bulk_environment", cause))?;
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live UI-affine interface with a writable output; the returned reference is
    // adopted before its status is interpreted.
    let status = unsafe { (core.vtable().Environment)(core.as_raw(), &mut raw) };
    adopt(raw, status, "bulk_environment")
}

/// Adopt a returned COM reference even when its getter also reports failure.
fn adopt<T: Interface>(
    raw: *mut c_void,
    status: HRESULT,
    operation: &'static str,
) -> Result<T, DesktopError> {
    // SAFETY: A non-null output is the one reference the getter transferred to this caller.
    let value = (!raw.is_null()).then(|| unsafe { T::from_raw(raw) });
    status.ok().map_err(|cause| DesktopError::new(operation, cause))?;
    value.ok_or_else(|| DesktopError::new(operation, windows::core::Error::from(E_POINTER)))
}

/// Answer every bulk-origin request synchronously on the UI apartment.
fn handler(
    events: Rc<Events>,
    transfers: Rc<Transfers>,
    environment: ICoreWebView2Environment,
) -> ICoreWebView2WebResourceRequestedEventHandler {
    WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let _callback = events.callback();
        let result = match args {
            Some(args) => serve(&environment, &transfers, &args),
            None => {
                Err(DesktopError::new("serve_bulk_origin", windows::core::Error::from(E_POINTER)))
            }
        };
        if let Err(cause) = result {
            events.fail(cause);
        }
        Ok(())
    }))
}

/// Route one request; page-controlled mismatches are 404s, native failures are errors.
fn serve(
    environment: &ICoreWebView2Environment,
    transfers: &Transfers,
    args: &ICoreWebView2WebResourceRequestedEventArgs,
) -> Result<(), DesktopError> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live callback arguments with a writable output; adopted before the status.
    let status = unsafe { (args.vtable().Request)(args.as_raw(), &mut raw) };
    let request: ICoreWebView2WebResourceRequest = adopt(raw, status, "bulk_request")?;
    // SAFETY: Each getter writes one CoTaskMem string owned by `native_text` on this UI thread.
    let uri = native_text(|output| unsafe { request.Uri(output) })?;
    if !under_origin(&uri) {
        // Other WebResourceRequested owners answer their own filtered requests.
        return Ok(());
    }
    let uri = ascii(&uri);
    // SAFETY: As above, for the request method.
    let method = ascii(&native_text(|output| unsafe { request.Method(output) })?);
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live request with a writable output; adopted before the status.
    let status = unsafe { (request.vtable().Headers)(request.as_raw(), &mut raw) };
    let headers: ICoreWebView2HttpRequestHeaders = adopt(raw, status, "bulk_request_headers")?;
    // SAFETY: A missing header fails the getter; any returned string is still owned and freed.
    let origin = native_text(|output| unsafe { headers.GetHeader(w!("Origin"), output) })
        .ok()
        .and_then(|origin| ascii(&origin));
    let route = match (&uri, &method) {
        (Some(uri), Some(method)) => {
            transfers.route(method.as_str(), uri.as_str(), origin.as_ref().map(Ascii::as_str))
        }
        _ => None,
    };
    let now = Instant::now();
    let response = match route {
        Some(Route::Download(token)) => match transfers.take_download(token, now) {
            Some(download) => response::message(environment, transfers.origin(), download)?,
            None => refused(environment, "unknown_download")?,
        },
        Some(Route::Upload(token)) => match transfers.begin_upload(token) {
            Some(slot) => match read(&request, transfers)
                .and_then(|buffer| transfers.finish_upload(slot, buffer, now))
            {
                Ok(()) => response::accepted(environment, transfers.origin())?,
                Err(refusal) => {
                    tracing::warn!(operation="bulk_upload_refused", refusal=?refusal,
                        "Bulk panel upload was refused and its body released");
                    response::missing(environment)?
                }
            },
            None => refused(environment, "unknown_upload")?,
        },
        None => refused(environment, "unmatched_request")?,
    };
    // SAFETY: The live callback arguments take this response on their own UI thread.
    unsafe { args.SetResponse(&response) }
        .map_err(|cause| DesktopError::new("respond_bulk_origin", cause))
}

/// Observe a page-controlled miss without echoing its URL or token.
fn refused(
    environment: &ICoreWebView2Environment,
    reason: &'static str,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    tracing::debug!(operation = "bulk_origin_miss", reason, "Bulk origin request matched nothing");
    response::missing(environment)
}

/// Read one request body into storage claimed from the session partition.
fn read(
    request: &ICoreWebView2WebResourceRequest,
    transfers: &Transfers,
) -> Result<UploadBuffer, Refusal> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live request with a writable output; adopted before the status.
    let status = unsafe { (request.vtable().Content)(request.as_raw(), &mut raw) };
    let stream: windows::Win32::System::Com::IStream =
        adopt(raw, status, "bulk_request_body").map_err(|_| Refusal::Read)?;
    let mut buffer = transfers.upload_buffer();
    while buffer.fill(|chunk| {
        let capacity = u32::try_from(chunk.len()).ok()?;
        let mut read = 0_u32;
        // SAFETY: `chunk` is owned writable storage of exactly `capacity` bytes, and the
        // optional count output is a live local for this synchronous call.
        let status = unsafe { stream.Read(chunk.as_mut_ptr().cast(), capacity, Some(&mut read)) };
        status.is_ok().then_some(read as usize)
    })? {}
    Ok(buffer)
}

/// Fixed ASCII storage for one short native string; nothing is allocated in the callback.
struct Ascii {
    bytes: [u8; MAX_TEXT_UNITS],
    len: usize,
}

impl Ascii {
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }
}

/// Own one returned native string before interpreting its getter status.
fn native_text(
    get: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>,
) -> Result<NativeString, DesktopError> {
    let mut pointer = PWSTR::null();
    let status = get(&mut pointer);
    let owned = NativeString(pointer);
    status.map_err(|cause| DesktopError::new("bulk_request_text", cause))?;
    if owned.0.is_null() {
        return Err(DesktopError::new("bulk_request_text", windows::core::Error::from(E_POINTER)));
    }
    Ok(owned)
}

/// Report whether a URL lies under the bulk origin without copying or bounding it.
fn under_origin(text: &NativeString) -> bool {
    crate::main_bulk::ORIGIN.bytes().enumerate().all(|(index, expected)| {
        // SAFETY: The string is NUL-terminated and the scan stops at its first mismatch; the
        // terminator mismatches every origin byte, so no read passes the end.
        let unit = unsafe { text.0.as_ptr().add(index).read() };
        unit == u16::from(expected)
    })
}

/// Copy a short ASCII string; longer or non-ASCII text cannot match and is `None`.
fn ascii(text: &NativeString) -> Option<Ascii> {
    let mut copy = Ascii { bytes: [0; MAX_TEXT_UNITS], len: 0 };
    for index in 0..=MAX_TEXT_UNITS {
        // SAFETY: The string is NUL-terminated; the scan stops at its terminator and never
        // reads past the first unit beyond the fixed bound.
        let unit = unsafe { text.0.as_ptr().add(index).read() };
        if unit == 0 {
            copy.len = index;
            return Some(copy);
        }
        match (u8::try_from(unit), copy.bytes.get_mut(index)) {
            (Ok(byte), Some(slot)) if byte.is_ascii() => *slot = byte,
            _ => return None,
        }
    }
    None
}
