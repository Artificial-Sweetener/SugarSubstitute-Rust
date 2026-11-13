// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Locate the composed page in the main window.
//!
//! The browser's bounds are relative to its controller's host window, which the main window
//! moves to where the page starts: below the caption strip while a caption peer needs it, at
//! the client's top otherwise. That host window's client origin is therefore the page's
//! top-left, the origin of every point WebView2 receives and of the browser's own visual.

use super::*;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_NON_CLIENT_REGION_KIND, COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION,
    ICoreWebView2CompositionController4,
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT},
    Graphics::Gdi::{ClientToScreen, ScreenToClient},
    UI::WindowsAndMessaging::HTCAPTION,
};

/// The controller's host window, whose client origin is the page's top-left.
///
/// # Errors
/// Preserves the controller's failure.
pub(super) fn host(controller: &ICoreWebView2Controller) -> Result<HWND, DesktopError> {
    let mut host = HWND::default();
    // SAFETY: The retained controller belongs to the calling UI apartment and copies one
    // handle into live local storage.
    unsafe { controller.ParentWindow(&raw mut host) }
        .map_err(|cause| DesktopError::new("locate_composed_page", cause))?;
    Ok(host)
}

/// The page's top-left in screen pixels.
///
/// # Errors
/// Fails when the host window no longer maps to the screen.
pub(super) fn screen_origin(host: HWND) -> Result<POINT, DesktopError> {
    let mut origin = POINT::default();
    // SAFETY: The host is a live window of this UI thread; the call writes local storage.
    if !unsafe { ClientToScreen(host, &raw mut origin) }.as_bool() {
        return Err(DesktopError::message("locate_composed_page", "Page origin is unavailable"));
    }
    Ok(origin)
}

/// The page's top-left in `window`'s client pixels.
///
/// # Errors
/// Fails when either window no longer maps to the screen.
pub(super) fn client_origin(host: HWND, window: HWND) -> Result<POINT, DesktopError> {
    let mut origin = screen_origin(host)?;
    // SAFETY: The main window is live on this UI thread; the call rewrites local storage.
    if !unsafe { ScreenToClient(window, &raw mut origin) }.as_bool() {
        return Err(DesktopError::message("locate_composed_page", "Client origin is unavailable"));
    }
    Ok(origin)
}

/// Translate a point whose space has its origin at `origin` into page pixels.
pub(super) const fn page_point(point: POINT, origin: POINT) -> POINT {
    POINT { x: point.x - origin.x, y: point.y - origin.y }
}

/// Answer a client-area hit test at the screen point in `lparam` over the page's caption
/// region with `HTCAPTION`, so Windows moves, snaps and maximizes the window. A
/// composition-hosted WebView2 owns no HWND and cannot answer `WM_NCHITTEST` for its own
/// `app-region: drag` elements; its regions are in page pixels.
///
/// # Errors
/// Fails when the point cannot be mapped into the page or the browser cannot be queried.
pub(super) fn caption(
    regions: &ICoreWebView2CompositionController4,
    host: HWND,
    lparam: LPARAM,
) -> Result<Option<LRESULT>, DesktopError> {
    let mut point =
        POINT { x: lparam.0 as u16 as i16 as i32, y: (lparam.0 >> 16) as u16 as i16 as i32 };
    // SAFETY: The live page host and local POINT belong to this UI-thread callback.
    if !unsafe { ScreenToClient(host, &raw mut point) }.as_bool() {
        return Err(DesktopError::message(
            "composed_browser_caption_coordinates",
            "Native hit-test coordinates are unavailable",
        ));
    }
    let mut kind = COREWEBVIEW2_NON_CLIENT_REGION_KIND::default();
    // SAFETY: The controller is retained on its creating UI apartment and the query writes
    // one scalar into live local storage.
    unsafe { regions.GetNonClientRegionAtPoint(point, &raw mut kind) }
        .map_err(|cause| DesktopError::new("query_composed_browser_caption", cause))?;
    Ok(caption_hit(kind))
}

/// Honor only the page's caption: the host draws its own window buttons and frame edges.
pub(super) fn caption_hit(kind: COREWEBVIEW2_NON_CLIENT_REGION_KIND) -> Option<LRESULT> {
    (kind == COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION).then_some(LRESULT(HTCAPTION as isize))
}
