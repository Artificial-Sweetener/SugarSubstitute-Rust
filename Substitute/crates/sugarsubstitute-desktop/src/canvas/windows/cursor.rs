// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one native window's canvas cursors: the shared Windows arrow, size and forbidden cursors,
//! DPI-sized Qt hands and the rendered editing cursors, and create private cursors from
//! straight RGBA images such as a drag-out's preview cursor.
//!
//! The standalone canvas peer and the composed main window each own one cache.

use super::*;
mod generated;
mod resource;

use ::windows::Win32::UI::{
    HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
    WindowsAndMessaging::{
        DestroyCursor, GetCursor, HCURSOR, IDC_ARROW, IDC_NO, IDC_SIZENESW, IDC_SIZENS,
        IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW, SM_CXCURSOR, SM_CYCURSOR, SetCursor,
    },
};
use resource::load_cursor;
#[cfg(feature = "native-composition")]
pub(crate) use resource::rgba_cursor;
#[cfg(test)]
use resource::select_frame;
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_canvas::{CursorIntent, ResizeDirection};

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

/// Windows-owned cursors of the current scheme: shared, never destroyed by the canvas.
#[derive(Clone, Copy)]
struct System {
    arrow: HCURSOR,
    horizontal: HCURSOR,
    vertical: HCURSOR,
    diagonal_down: HCURSOR,
    diagonal_up: HCURSOR,
    forbidden: HCURSOR,
}

impl System {
    /// Load the scheme's arrow, size and forbidden cursors.
    fn load() -> Result<Self, Box<dyn Error>> {
        // SAFETY: Each IDC_* identifier requests a shared Windows cursor that callers never
        // destroy, so no handle loaded here is private.
        unsafe {
            Ok(Self {
                arrow: LoadCursorW(None, IDC_ARROW)?,
                horizontal: LoadCursorW(None, IDC_SIZEWE)?,
                vertical: LoadCursorW(None, IDC_SIZENS)?,
                diagonal_down: LoadCursorW(None, IDC_SIZENWSE)?,
                diagonal_up: LoadCursorW(None, IDC_SIZENESW)?,
                forbidden: LoadCursorW(None, IDC_NO)?,
            })
        }
    }

    /// The shared cursor a system intent selects; every other intent shows the arrow.
    const fn cursor(self, intent: CursorIntent) -> HCURSOR {
        match intent {
            CursorIntent::Resize(ResizeDirection::Horizontal) => self.horizontal,
            CursorIntent::Resize(ResizeDirection::Vertical) => self.vertical,
            CursorIntent::Resize(ResizeDirection::DiagonalDown) => self.diagonal_down,
            CursorIntent::Resize(ResizeDirection::DiagonalUp) => self.diagonal_up,
            CursorIntent::Forbidden => self.forbidden,
            _ => self.arrow,
        }
    }
}

struct Cache {
    system: System,
    pair: Option<CursorPair>,
    rendered: generated::Rendered,
}

/// A window's physical DPI and the native cursor size Windows uses at that DPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Metrics {
    dpi: u32,
    size: [i32; 2],
}

/// One current-DPI hand pair, recently rendered editing cursors and the shared system arrow for
/// one native window.
pub(crate) struct Cursors(RefCell<Cache>);

impl Cursors {
    /// Create the shared arrow and private hands using this live peer's physical DPI.
    /// Errors preserve native metric/resource failures; call on the peer's owning thread.
    pub(crate) fn new(window: HWND) -> Result<Self, Box<dyn Error>> {
        Self::sized(metrics(window)?)
    }

    /// Create the shared arrow and private hands at one physical cursor size.
    fn sized(metrics: Metrics) -> Result<Self, Box<dyn Error>> {
        let system = System::load()?;
        let pair = CursorPair::new(metrics.size)?;
        Ok(Self(RefCell::new(Cache {
            system,
            pair: Some(pair),
            rendered: generated::Rendered::default(),
        })))
    }

    /// Resolve an intent, refreshing size if DPI changed; retain the returned private resource.
    /// Errors preserve native resource failures. Coordinates and dimensions are physical pixels.
    pub(crate) fn get(
        &self,
        window: HWND,
        intent: CursorIntent,
    ) -> Result<CursorResource, Box<dyn Error>> {
        self.resolve(metrics(window)?, intent)
    }

    /// Resolve an intent at the window's current DPI, rebuilding the hand pair when the native
    /// cursor size changed and rendering editing cursors at that DPI.
    fn resolve(
        &self,
        metrics: Metrics,
        intent: CursorIntent,
    ) -> Result<CursorResource, Box<dyn Error>> {
        self.resize(metrics, false)?;
        if let Some(art) = generated::art(intent) {
            let cursor = self.0.borrow_mut().rendered.get(art, metrics.dpi)?;
            return Ok(CursorResource::Owned(cursor));
        }
        let cache = self.0.borrow();
        let pair = cache.pair.as_ref().ok_or_else(missing_hands)?;
        Ok(match intent {
            CursorIntent::OpenHand => CursorResource::Owned(pair.open.clone()),
            CursorIntent::ClosedHand => CursorResource::Owned(pair.closed.clone()),
            _ => CursorResource::Shared(cache.system.cursor(intent)),
        })
    }

    /// Reload the current Windows arrow and rebuild exactly one DPI-sized private hand pair.
    pub(super) fn refresh(&self, window: HWND, force: bool) -> Result<(), Box<dyn Error>> {
        self.resize(metrics(window)?, force)
    }

    fn resize(&self, metrics: Metrics, force: bool) -> Result<(), Box<dyn Error>> {
        self.0.borrow_mut().rendered.keep_dpi(metrics.dpi);
        if !force && self.0.borrow().pair.as_ref().is_some_and(|pair| pair.size == metrics.size) {
            return Ok(());
        }
        let pair = CursorPair::new(metrics.size)?;
        let system = if force { System::load()? } else { self.0.borrow().system };
        let mut cache = self.0.borrow_mut();
        cache.system = system;
        cache.pair = Some(pair);
        Ok(())
    }

    /// Borrow the Windows-owned arrow for final private-cursor deselection.
    pub(crate) fn arrow(&self) -> HCURSOR {
        self.0.borrow().system.arrow
    }

    /// Release cached private cursors after the peer's selected and desired resources have
    /// retired.
    pub(crate) fn retire_custom(&self) {
        let mut cache = self.0.borrow_mut();
        cache.pair = None;
        cache.rendered.clear();
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

fn metrics(window: HWND) -> Result<Metrics, DesktopError> {
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
    Ok(Metrics { dpi, size })
}

fn missing_hands() -> DesktopError {
    DesktopError::message("canvas_cursor_retired", "Native canvas hand cursors were retired")
}

#[cfg(test)]
#[path = "cursor/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cursor/rendered_tests.rs"]
mod rendered_tests;
