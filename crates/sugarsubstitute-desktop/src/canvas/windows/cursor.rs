// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own DPI-sized Qt hand cursors and the shared Windows arrow for one native window.
//!
//! The standalone canvas peer and the composed main window each own one cache.

use super::*;
use ::windows::Win32::UI::{
    HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
    WindowsAndMessaging::{
        CreateIconFromResourceEx, DestroyCursor, GetCursor, HCURSOR, IDC_ARROW, LR_DEFAULTCOLOR,
        LoadCursorW, SM_CXCURSOR, SM_CYCURSOR, SetCursor,
    },
};
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_canvas::CursorIntent;

const CURSOR_RESOURCE_VERSION: u32 = 0x0003_0000;
const CURSOR_ENTRIES: usize = 3;
const DIRECTORY_BYTES: usize = 6 + CURSOR_ENTRIES * 16;
const OPEN_HAND: &[u8] = include_bytes!("../../../assets/cursors/qt-open-hand.cur");
const CLOSED_HAND: &[u8] = include_bytes!("../../../assets/cursors/qt-closed-hand.cur");

/// Private native cursor; its final owner must release it only after native selection ends.
pub(crate) struct OwnedCursor(HCURSOR);

impl OwnedCursor {
    /// Borrow the handle without transferring native ownership.
    pub(crate) const fn handle(&self) -> HCURSOR {
        self.0
    }
}

impl Drop for OwnedCursor {
    fn drop(&mut self) {
        // SAFETY: This handle was created privately with CreateIconFromResourceEx(fIcon=false),
        // and State restores a shared arrow before releasing any currently selected last owner.
        if let Err(cause) = unsafe { DestroyCursor(self.0) } {
            tracing::error!(operation="canvas_cursor_retirement", cause=%cause,
                "Native canvas cursor retirement failed");
        }
    }
}

#[derive(Clone)]
/// Shared OS cursor or retained private cursor, confined to the peer's native thread.
pub(crate) enum CursorResource {
    Shared(HCURSOR),
    Owned(Rc<OwnedCursor>),
}

impl CursorResource {
    /// Borrow the handle for synchronous Windows cursor selection.
    pub(crate) fn handle(&self) -> HCURSOR {
        match self {
            Self::Shared(cursor) => *cursor,
            Self::Owned(cursor) => cursor.handle(),
        }
    }

    /// Retain private cursor custody across cache refresh and native selection changes.
    pub(crate) fn owned(&self) -> Option<Rc<OwnedCursor>> {
        match self {
            Self::Shared(_) => None,
            Self::Owned(cursor) => Some(cursor.clone()),
        }
    }
    /// Reassert this resource only after an actual native override, retaining private custody.
    pub(crate) fn select(&self) -> Option<Rc<OwnedCursor>> {
        let desired = self.handle();
        // SAFETY: Read and selection are synchronous on the native window's owning UI thread.
        select_when_changed(
            desired,
            // SAFETY: This read only observes the current cursor on the window's UI thread.
            || unsafe { GetCursor() },
            // SAFETY: The retained resource remains live through native selection.
            |cursor| unsafe {
                SetCursor(Some(cursor));
            },
        );
        self.owned()
    }
}

/// Select a desired cursor only when the actual native cursor differs.
/// The caller retains resource custody independently of whether selection was necessary.
pub(super) fn select_when_changed<T: Copy + Eq>(
    desired: T,
    current: impl FnOnce() -> T,
    select: impl FnOnce(T),
) {
    if current() != desired {
        select(desired);
    }
}

struct CursorPair {
    size: [i32; 2],
    open: Rc<OwnedCursor>,
    closed: Rc<OwnedCursor>,
}

struct Cache {
    arrow: HCURSOR,
    pair: Option<CursorPair>,
}

/// One current-DPI custom pair plus the shared system arrow for one native window.
pub(crate) struct Cursors(RefCell<Cache>);

impl Cursors {
    /// Create the shared arrow and private hands using this live peer's physical DPI.
    /// Errors preserve native metric/resource failures; call on the peer's owning thread.
    pub(crate) fn new(window: HWND) -> Result<Self, Box<dyn Error>> {
        // SAFETY: IDC_ARROW requests a shared Windows cursor that callers never destroy.
        let arrow = unsafe { LoadCursorW(None, IDC_ARROW)? };
        let size = cursor_size(window)?;
        let pair = CursorPair::new(size)?;
        Ok(Self(RefCell::new(Cache { arrow, pair: Some(pair) })))
    }

    /// Resolve an intent, refreshing size if DPI changed; retain the returned private resource.
    /// Errors preserve native resource failures. Coordinates and dimensions are physical pixels.
    pub(crate) fn get(
        &self,
        window: HWND,
        intent: CursorIntent,
    ) -> Result<CursorResource, Box<dyn Error>> {
        self.refresh(window, false)?;
        let cache = self.0.borrow();
        let pair = cache.pair.as_ref().ok_or_else(invalid_cursor)?;
        Ok(match intent {
            CursorIntent::Arrow => CursorResource::Shared(cache.arrow),
            CursorIntent::OpenHand => CursorResource::Owned(pair.open.clone()),
            CursorIntent::ClosedHand => CursorResource::Owned(pair.closed.clone()),
        })
    }

    /// Reload the current Windows arrow and rebuild exactly one DPI-sized private hand pair.
    pub(super) fn refresh(&self, window: HWND, force: bool) -> Result<(), Box<dyn Error>> {
        let size = cursor_size(window)?;
        if !force && self.0.borrow().pair.as_ref().is_some_and(|pair| pair.size == size) {
            return Ok(());
        }
        let pair = CursorPair::new(size)?;
        let arrow = if force {
            // SAFETY: Reload the shared scheme-owned arrow; neither old nor new handle is private.
            unsafe { LoadCursorW(None, IDC_ARROW)? }
        } else {
            self.0.borrow().arrow
        };
        *self.0.borrow_mut() = Cache { arrow, pair: Some(pair) };
        Ok(())
    }

    /// Borrow the Windows-owned arrow for final private-cursor deselection.
    pub(crate) fn arrow(&self) -> HCURSOR {
        self.0.borrow().arrow
    }

    /// Release cached hands after the peer's selected and desired resources have retired.
    pub(crate) fn retire_custom(&self) {
        self.0.borrow_mut().pair = None;
    }
}

impl CursorPair {
    fn new(size: [i32; 2]) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            size,
            open: Rc::new(load_cursor(OPEN_HAND, size)?),
            closed: Rc::new(load_cursor(CLOSED_HAND, size)?),
        })
    }
}

fn cursor_size(window: HWND) -> Result<[i32; 2], DesktopError> {
    // SAFETY: Read only the live peer's physical DPI and the corresponding native metrics.
    let dpi = unsafe { GetDpiForWindow(window) };
    // SAFETY: Metric queries copy physical dimensions and retain no window or resource.
    let size = unsafe {
        [GetSystemMetricsForDpi(SM_CXCURSOR, dpi), GetSystemMetricsForDpi(SM_CYCURSOR, dpi)]
    };
    if dpi == 0 || size.iter().any(|edge| *edge <= 0) {
        return Err(DesktopError::message(
            "canvas_cursor_metrics",
            "Native cursor dimensions are unavailable",
        ));
    }
    Ok(size)
}

fn load_cursor(bytes: &[u8], size: [i32; 2]) -> Result<OwnedCursor, Box<dyn Error>> {
    let frame = select_frame(bytes, size)?;
    let mut resource = Vec::new();
    resource
        .try_reserve_exact(frame.bytes.len() + 4)
        .map_err(|cause| DesktopError::new("canvas_cursor_resource", cause))?;
    resource.extend_from_slice(&frame.hotspot[0].to_le_bytes());
    resource.extend_from_slice(&frame.hotspot[1].to_le_bytes());
    resource.extend_from_slice(frame.bytes);
    // SAFETY: The bounded parser proved a complete cursor DIB and hotspot prefix. Windows copies
    // the resource synchronously and returns a private cursor scaled to the requested native size.
    let icon = unsafe {
        CreateIconFromResourceEx(
            &resource,
            false,
            CURSOR_RESOURCE_VERSION,
            size[0],
            size[1],
            LR_DEFAULTCOLOR,
        )?
    };
    Ok(OwnedCursor(HCURSOR(icon.0)))
}

#[derive(Clone, Copy)]
struct CursorFrame<'a> {
    extent: [u16; 2],
    hotspot: [u16; 2],
    bytes: &'a [u8],
}

fn select_frame(bytes: &[u8], requested: [i32; 2]) -> Result<CursorFrame<'_>, DesktopError> {
    if bytes.len() < DIRECTORY_BYTES
        || word(bytes, 0) != Some(0)
        || word(bytes, 2) != Some(2)
        || word(bytes, 4) != Some(CURSOR_ENTRIES as u16)
        || requested.iter().any(|edge| *edge <= 0)
    {
        return Err(invalid_cursor());
    }
    let mut selected = None;
    let mut selected_distance = u64::MAX;
    for index in 0..CURSOR_ENTRIES {
        let entry = 6 + index * 16;
        let width = extent_byte(bytes[entry]);
        let height = extent_byte(bytes[entry + 1]);
        if bytes[entry + 2] != 0 || bytes[entry + 3] != 0 {
            return Err(invalid_cursor());
        }
        let hotspot = [
            word(bytes, entry + 4).ok_or_else(invalid_cursor)?,
            word(bytes, entry + 6).ok_or_else(invalid_cursor)?,
        ];
        let length = usize::try_from(dword(bytes, entry + 8).ok_or_else(invalid_cursor)?)
            .map_err(|_| invalid_cursor())?;
        let offset = usize::try_from(dword(bytes, entry + 12).ok_or_else(invalid_cursor)?)
            .map_err(|_| invalid_cursor())?;
        let end = offset.checked_add(length).ok_or_else(invalid_cursor)?;
        if offset < DIRECTORY_BYTES || length == 0 || end > bytes.len() {
            return Err(invalid_cursor());
        }
        let frame = CursorFrame { extent: [width, height], hotspot, bytes: &bytes[offset..end] };
        validate_dib(frame)?;
        let requested = [
            requested[0].min(i32::from(u16::MAX)) as u16,
            requested[1].min(i32::from(u16::MAX)) as u16,
        ];
        let distance =
            u64::from(width.abs_diff(requested[0])) + u64::from(height.abs_diff(requested[1]));
        let prefer_larger = selected.is_some_and(|current: CursorFrame<'_>| {
            distance == selected_distance && width > current.extent[0]
        });
        if distance < selected_distance || prefer_larger {
            selected = Some(frame);
            selected_distance = distance;
        }
    }
    selected.ok_or_else(invalid_cursor)
}

fn validate_dib(frame: CursorFrame<'_>) -> Result<(), DesktopError> {
    let header = dword(frame.bytes, 0).ok_or_else(invalid_cursor)?;
    let width = dword(frame.bytes, 4).ok_or_else(invalid_cursor)?;
    let height = dword(frame.bytes, 8).ok_or_else(invalid_cursor)?;
    let planes = word(frame.bytes, 12).ok_or_else(invalid_cursor)?;
    let bits = word(frame.bytes, 14).ok_or_else(invalid_cursor)?;
    let compression = dword(frame.bytes, 16).ok_or_else(invalid_cursor)?;
    let xor = usize::from(frame.extent[0])
        .checked_mul(usize::from(frame.extent[1]))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(invalid_cursor)?;
    let mask_stride = usize::from(frame.extent[0]).div_ceil(32) * 4;
    let required = 40_usize
        .checked_add(xor)
        .and_then(|bytes| bytes.checked_add(mask_stride * usize::from(frame.extent[1])))
        .ok_or_else(invalid_cursor)?;
    if header != 40
        || width != u32::from(frame.extent[0])
        || height != u32::from(frame.extent[1]) * 2
        || planes != 1
        || bits != 32
        || compression != 0
        || frame.hotspot[0] >= frame.extent[0]
        || frame.hotspot[1] >= frame.extent[1]
        || frame.bytes.len() < required
    {
        return Err(invalid_cursor());
    }
    Ok(())
}

const fn extent_byte(value: u8) -> u16 {
    if value == 0 { 256 } else { value as u16 }
}

fn word(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*bytes.get(offset)?, *bytes.get(offset + 1)?]))
}

fn dword(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

fn invalid_cursor() -> DesktopError {
    DesktopError::message("canvas_cursor_asset", "Embedded Qt cursor resource is invalid")
}

#[cfg(test)]
#[path = "cursor/tests.rs"]
mod tests;
