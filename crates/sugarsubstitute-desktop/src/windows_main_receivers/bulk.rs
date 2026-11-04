// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve the bulk channel's reserved path on the connected frontend origin to its panel document.
//!
//! The one request filter covers exactly the endpoint's prefix (origin, base path and reserved
//! path) in every resource context, so WebView2 raises `WebResourceRequested` on this UI
//! apartment for each request under it before any network access. This adapter is the
//! browser's only `WebResourceRequested` owner, and it answers every request it is shown
//! synchronously and locally: native getters return before the transfer registry is borrowed,
//! a request body is read in claimed bounded steps, and anything that is not a live single-use
//! token — another context, method, path, origin or token, or a failed native read — receives
//! an empty 404 while no body is retained. Each such refusal is recorded as one closed
//! [`crate::bulk_channel_diagnostic::Record`] naming the failed check, the raw resource context,
//! the method class and the `Origin` relation, never the URL, token or body. No request under
//! the prefix reaches the network.

mod response;

use super::*;
use crate::bulk_channel_diagnostic::{self, Method};
use crate::main_bulk::{
    Context, Endpoint, Header, Refusal, Request, Route, Transfers, UploadBuffer,
};
use std::{ffi::c_void, time::Instant};
use webview2_com::WebResourceRequestedEventHandler;
use windows::{
    Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    core::{BOOL, HRESULT, w},
};

/// Longest URL, method or `Origin` header the bulk channel can match, in UTF-16 units.
const MAX_TEXT_UNITS: usize = Endpoint::MAX_ROUTE_BYTES;

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

/// Retain the bulk channel's handler and filter registrations through checked removal.
///
/// The filter is fixed for this browser lifetime: its main lifetime serves one connected
/// origin, and a different connection installs its own filter in a new lifetime.
pub(super) struct BulkChannel {
    transfers: Rc<Transfers>,
    /// The exact filter string registered and later removed.
    filter: HSTRING,
    handler: Option<i64>,
    filtered: bool,
    uncertain: bool,
}

impl BulkChannel {
    pub(super) fn new(transfers: Rc<Transfers>) -> Self {
        let filter = HSTRING::from(transfers.endpoint().filter());
        Self { transfers, filter, handler: None, filtered: false, uncertain: false }
    }

    /// Install the handler before the filter, one native registration per call.
    ///
    /// The filter covers every resource context so a reserved-path image, navigation or other
    /// request is also answered locally instead of reaching the network.
    pub(super) fn install(
        &mut self,
        core: &ICoreWebView2,
        events: &Rc<Events>,
    ) -> Result<bool, DesktopError> {
        if self.uncertain {
            return Err(DesktopError::message("install_bulk_channel", "registration is uncertain"));
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
            result.map_err(|cause| DesktopError::new("install_bulk_channel", cause))?;
            return Ok(false);
        }
        if !self.filtered {
            // SAFETY: The retained bounded filter outlives this synchronous call on the UI thread.
            let result = unsafe {
                core.AddWebResourceRequestedFilter(
                    &self.filter,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
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
            // SAFETY: The exact filter registered above is removed on its creating UI thread.
            unsafe {
                core.RemoveWebResourceRequestedFilter(
                    &self.filter,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
            }
            .map_err(|cause| DesktopError::new("remove_bulk_filter", cause))?;
            self.filtered = false;
            return Ok(false);
        }
        if let Some(token) = self.handler {
            // SAFETY: This exact token belongs to the retained core on its UI thread.
            unsafe { core.remove_WebResourceRequested(token) }
                .map_err(|cause| DesktopError::new("remove_bulk_channel", cause))?;
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

/// Answer every filtered request synchronously and locally on the UI apartment.
///
/// A native failure while observing or answering a request still answers it with a 404 before
/// the failure is reported, so the request never falls through to the network.
fn handler(
    events: Rc<Events>,
    transfers: Rc<Transfers>,
    environment: ICoreWebView2Environment,
) -> ICoreWebView2WebResourceRequestedEventHandler {
    WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let _callback = events.callback();
        let Some(args) = args else {
            events.fail(DesktopError::new(
                "serve_bulk_channel",
                windows::core::Error::from(E_POINTER),
            ));
            return Ok(());
        };
        let response = serve(&environment, &transfers, &args).or_else(|cause| {
            events.fail(cause);
            response::missing(&environment)
        });
        let answered = response.and_then(|response| {
            // SAFETY: The live callback arguments take this response on their own UI thread.
            unsafe { args.SetResponse(&response) }
                .map_err(|cause| DesktopError::new("respond_bulk_channel", cause))
        });
        if let Err(cause) = answered {
            events.fail(cause);
        }
        Ok(())
    }))
}

/// Choose the response for one request; page-controlled mismatches are 404s, native failures
/// are errors the caller still answers with a 404.
fn serve(
    environment: &ICoreWebView2Environment,
    transfers: &Transfers,
    args: &ICoreWebView2WebResourceRequestedEventArgs,
) -> Result<ICoreWebView2WebResourceResponse, DesktopError> {
    let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
    // SAFETY: Live callback arguments with a writable scalar output on this UI thread.
    unsafe { args.ResourceContext(&mut context) }
        .map_err(|cause| DesktopError::new("bulk_request_context", cause))?;
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live callback arguments with a writable output; adopted before the status.
    let status = unsafe { (args.vtable().Request)(args.as_raw(), &mut raw) };
    let request: ICoreWebView2WebResourceRequest = adopt(raw, status, "bulk_request")?;
    // SAFETY: Each getter writes one CoTaskMem string owned by `native_text` on this UI thread.
    let uri = ascii(&native_text(|output| unsafe { request.Uri(output) })?);
    // SAFETY: As above, for the request method.
    let method = ascii(&native_text(|output| unsafe { request.Method(output) })?);
    let mut raw = std::ptr::null_mut();
    // SAFETY: Live request with a writable output; adopted before the status.
    let status = unsafe { (request.vtable().Headers)(request.as_raw(), &mut raw) };
    let headers: ICoreWebView2HttpRequestHeaders = adopt(raw, status, "bulk_request_headers")?;
    let origin = origin_header(&headers)?;
    let request_view = Request {
        context: classify_context(context),
        method: method.as_ref().map(Ascii::as_str),
        uri: uri.as_ref().map(Ascii::as_str),
        origin: match &origin {
            Some(Some(origin)) => Header::Text(origin.as_str()),
            Some(None) => Header::Unreadable,
            None => Header::Absent,
        },
    };
    let refuse = |miss| {
        bulk_channel_diagnostic::Record {
            miss,
            context: context.0,
            method: Method::classify(request_view.method),
            origin: transfers.endpoint().origin(request_view.origin),
        }
        .emit();
        response::missing(environment)
    };
    let now = Instant::now();
    match transfers.endpoint().route(request_view) {
        Ok(Route::Download(token)) => match transfers.take_download(token, now) {
            Ok(download) => response::message(environment, download),
            Err(miss) => refuse(miss),
        },
        Ok(Route::Upload(token)) => match transfers.begin_upload(token) {
            Ok(slot) => match read(&request, transfers)
                .and_then(|buffer| transfers.finish_upload(slot, buffer, now))
            {
                Ok(()) => response::accepted(environment),
                Err(refusal) => refuse(refusal.upload_miss()),
            },
            Err(miss) => refuse(miss),
        },
        Err(miss) => refuse(miss),
    }
}

/// Classify WebView2's resource context for the endpoint's script-request check.
///
/// WebView2 raises the bridge's own Fetch API calls with
/// `COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST` (7), not `..._FETCH` (8): a production
/// host refused every bulk download as a non-script request until both were admitted. Every
/// other context, such as a document, frame or image load, stays refused.
fn classify_context(context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT) -> Context {
    match context {
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => Context::Fetch,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST => Context::XmlHttpRequest,
        _ => Context::Other,
    }
}

/// Read the `Origin` header: `None` when absent, `Some(None)` when present but unreadable.
fn origin_header(
    headers: &ICoreWebView2HttpRequestHeaders,
) -> Result<Option<Option<Ascii>>, DesktopError> {
    let mut present = BOOL(0);
    // SAFETY: Live headers with a writable scalar output on this UI thread.
    unsafe { headers.Contains(w!("Origin"), &mut present) }
        .map_err(|cause| DesktopError::new("bulk_request_origin", cause))?;
    if !present.as_bool() {
        return Ok(None);
    }
    // SAFETY: The getter writes one CoTaskMem string owned by `native_text` on this UI thread.
    let origin = native_text(|output| unsafe { headers.GetHeader(w!("Origin"), output) })
        .ok()
        .and_then(|origin| ascii(&origin));
    Ok(Some(origin))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_webview2_reports_for_the_bridge_fetch_is_a_script_request() {
        // Observed on the production host: the bridge's same-origin Fetch API call arrives as 7.
        assert_eq!(COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST.0, 7);
        assert_eq!(classify_context(COREWEBVIEW2_WEB_RESOURCE_CONTEXT(7)), Context::XmlHttpRequest);
        assert_eq!(classify_context(COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH), Context::Fetch);
    }

    #[test]
    fn navigation_and_subresource_contexts_are_never_script_requests() {
        for context in [
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_EVENT_SOURCE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MANIFEST,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SIGNED_EXCHANGE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_CSP_VIOLATION_REPORT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_OTHER,
        ] {
            assert_eq!(classify_context(context), Context::Other, "{context:?}");
        }
    }
}
