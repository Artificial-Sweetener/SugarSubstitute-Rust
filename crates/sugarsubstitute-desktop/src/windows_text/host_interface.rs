// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose the real system ITextHost identity while preserving generated ABI signatures.
use super::host::State;
use std::rc::Rc;
use sugarsubstitute_ui::windows::text_style::TextStyle;
use windows::{
    Win32::{
        Foundation::{COLORREF, E_INVALIDARG, E_NOTIMPL, LPARAM, POINT, RECT, SIZE, WPARAM},
        Graphics::Gdi::{
            COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOWTEXT, GetSysColor, HBITMAP, HDC,
            HRGN, SYS_COLOR_INDEX,
        },
        UI::{
            Controls::RichEdit::*,
            Input::{
                Ime::{HIMC, ImmGetContext, ImmReleaseContext},
                KeyboardAndMouse::{GetFocus, SetFocus},
            },
            WindowsAndMessaging::*,
        },
    },
    core::{BOOL, GUID, Interface, Result, implement},
};

windows::core::imp::define_interface!(
    HostInterface,
    HostInterfaceVtbl,
    0x13e670f4_1a5a_11cf_abeb_00aa00b65ea1
);
windows::core::imp::interface_hierarchy!(HostInterface, windows::core::IUnknown);
impl windows::core::RuntimeName for HostInterface {}
/// The system contract has the generated ITextHost vtable layout and its real exported IID.
#[repr(transparent)]
pub struct HostInterfaceVtbl(ITextHost_Vtbl);
impl HostInterfaceVtbl {
    /// Build the exact native callback table without changing a single method ABI.
    const fn new<Identity: ITextHost_Impl, const OFFSET: isize>() -> Self {
        Self(ITextHost_Vtbl::new::<Identity, OFFSET>())
    }
    /// Generated Windows metadata assigns GUID zero to this dynamically exported interface.
    fn matches(iid: &GUID) -> bool {
        *iid == HostInterface::IID
    }
}
#[path = "accessibility/host_information.rs"]
mod information;
#[implement(HostInterface, information::Information, Agile = false)]
struct TextHost {
    state: Rc<State>,
}
/// Retain one COM host object before calling the reentrant native factory.
pub(super) fn create(state: Rc<State>) -> HostInterface {
    TextHost { state }.into()
}

impl ITextHost_Impl for TextHost_Impl {
    fn TxGetDC(&self) -> HDC {
        self.state.dc()
    }
    fn TxReleaseDC(&self, dc: HDC) -> i32 {
        self.state.release_dc(dc)
    }
    fn TxShowScrollBar(&self, bar: i32, show: BOOL) -> BOOL {
        if self.state.style == TextStyle::Field
            && (bar == SB_HORZ.0 || bar == SB_VERT.0 || bar == SB_BOTH.0)
        {
            self.state.invalidate();
            return true.into();
        }
        if bar != SB_VERT.0 && bar != SB_BOTH.0 {
            if bar == SB_HORZ.0 && !show.as_bool() {
                return true.into();
            }
            self.state.fail("console_host_scroll_axis", windows::core::Error::from(E_INVALIDARG));
            return false.into();
        }
        self.state.scroll_visible.set(show.as_bool());
        self.state.invalidate();
        true.into()
    }
    fn TxEnableScrollBar(&self, bar: SCROLLBAR_CONSTANTS, _arrows: i32) -> BOOL {
        if bar == SB_VERT
            || bar == SB_BOTH
            || (bar == SB_HORZ && self.state.style == TextStyle::Field)
        {
            return true.into();
        }
        self.state.fail("console_host_scroll_axis", windows::core::Error::from(E_INVALIDARG));
        false.into()
    }
    fn TxSetScrollRange(&self, bar: i32, min: i32, max: i32, _redraw: BOOL) -> BOOL {
        if bar == SB_HORZ.0 && max >= min && self.state.style == TextStyle::Field {
            self.state.invalidate();
            return true.into();
        }
        if bar != SB_VERT.0 || max < min {
            self.state.fail("console_host_scroll_range", windows::core::Error::from(E_INVALIDARG));
            return false.into();
        }
        self.state.scroll_min.set(min);
        self.state.scroll_max.set(max);
        self.state.invalidate();
        true.into()
    }
    fn TxSetScrollPos(&self, bar: i32, position: i32, _redraw: BOOL) -> BOOL {
        if bar == SB_HORZ.0 && self.state.style == TextStyle::Field {
            self.state.invalidate();
            return true.into();
        }
        if bar != SB_VERT.0 {
            self.state.fail("console_host_scroll_axis", windows::core::Error::from(E_INVALIDARG));
            return false.into();
        }
        self.state.scroll_position.set(position);
        self.state.invalidate();
        true.into()
    }
    fn TxInvalidateRect(&self, _rect: *mut RECT, _erase: BOOL) {
        self.state.invalidate();
    }
    fn TxViewChange(&self, _update: BOOL) {
        self.state.invalidate();
    }
    fn TxCreateCaret(&self, bitmap: HBITMAP, width: i32, height: i32) -> BOOL {
        self.state.caret_extent.set((width, height));
        if self.state.style == TextStyle::Field
            && let Err(cause) = self.state.caret.create(bitmap, width, height)
        {
            self.state.record(cause.into());
            return false.into();
        }
        true.into()
    }
    fn TxShowCaret(&self, show: BOOL) -> BOOL {
        self.state.caret_visible.set(show.as_bool());
        if self.state.style == TextStyle::Field
            && let Err(cause) = self.state.caret.show(show.as_bool())
        {
            self.state.record(cause.into());
            return false.into();
        }
        true.into()
    }
    fn TxSetCaretPos(&self, x: i32, y: i32) -> BOOL {
        self.state.caret_position.set((x, y));
        if self.state.style == TextStyle::Field
            && let Err(cause) = self.state.caret.position(x, y)
        {
            self.state.record(cause.into());
            return false.into();
        }
        true.into()
    }
    fn TxSetTimer(&self, id: u32, period_ms: u32) -> BOOL {
        self.state.set_timer(id as usize, period_ms)
    }
    fn TxKillTimer(&self, id: u32) {
        if let Err(cause) = self.state.kill_timer(id as usize) {
            self.state.fail("console_host_timer_remove", cause);
        }
    }
    fn TxScrollWindowEx(
        &self,
        _dx: i32,
        _dy: i32,
        _scroll: *mut RECT,
        _clip: *mut RECT,
        _region: HRGN,
        update: *mut RECT,
        _flags: SCROLL_WINDOW_FLAGS,
    ) {
        if !update.is_null() {
            // SAFETY: The synchronous system callback lends this writable RECT.
            unsafe {
                update.write(self.state.bounds.get());
            }
        }
        self.state.invalidate();
    }
    fn TxSetCapture(&self, capture: BOOL) {
        let result =
            if capture.as_bool() { self.state.capture() } else { self.state.release_capture() };
        if let Err(cause) = result {
            self.state.record(cause);
        }
    }
    fn TxSetFocus(&self) {
        // SAFETY: Only this original retained affine input window may receive focus.
        unsafe {
            let _previous = SetFocus(Some(self.state.hwnd));
            if GetFocus() != self.state.hwnd {
                self.state.fail("console_host_focus", windows::core::Error::from_thread());
            }
        }
    }
    fn TxSetCursor(&self, cursor: HCURSOR, _text: BOOL) {
        // SAFETY: The native system cursor handle is synchronously borrowed.
        unsafe {
            SetCursor(Some(cursor));
        }
    }
    fn TxScreenToClient(&self, point: *mut POINT) -> BOOL {
        if point.is_null() {
            return false.into();
        }
        // SAFETY: The synchronous native callback lends one writable point for this exact child.
        unsafe { windows::Win32::Graphics::Gdi::ScreenToClient(self.state.hwnd, point) }
    }
    fn TxClientToScreen(&self, point: *mut POINT) -> BOOL {
        if point.is_null() {
            return false.into();
        }
        // SAFETY: The synchronous native callback lends one writable point for this exact child.
        unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(self.state.hwnd, point) }
    }
    fn TxActivate(&self, previous: *mut i32) -> Result<()> {
        if previous.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: The native callback lends the documented writable previous-state output.
        unsafe {
            previous.write(self.state.active.replace(1));
        }
        Ok(())
    }
    fn TxDeactivate(&self, state: i32) -> Result<()> {
        self.state.active.set(state);
        Ok(())
    }
    fn TxGetClientRect(&self, rect: *mut RECT) -> Result<()> {
        write(rect, self.state.client_bounds())
    }
    fn TxGetViewInset(&self, rect: *mut RECT) -> Result<()> {
        write(rect, self.state.style.inset())
    }
    fn TxGetCharFormat(&self, format: *const *const CHARFORMATW) -> Result<()> {
        if format.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: The stable host owns this format until after the native document retires.
        unsafe {
            format.cast_mut().write(self.state.font.as_ptr().cast());
        }
        Ok(())
    }
    fn TxGetParaFormat(&self, format: *const *const PARAFORMAT) -> Result<()> {
        if format.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: The stable host owns this paragraph until after the native document retires.
        unsafe {
            format.cast_mut().write(self.state.paragraph.as_ptr());
        }
        Ok(())
    }
    fn TxGetSysColor(&self, index: SYS_COLOR_INDEX) -> COLORREF {
        if self.state.painting.get() {
            if self.state.style == TextStyle::Transcript && index == COLOR_HIGHLIGHT {
                return COLORREF(self.state.selection_background.get());
            }
            if self.state.style == TextStyle::Transcript && index == COLOR_HIGHLIGHTTEXT {
                return COLORREF(self.state.selection_foreground.get());
            }
            if index == COLOR_WINDOWTEXT {
                return COLORREF(self.state.default_foreground.get());
            }
        }
        // SAFETY: Read a current system palette fact without retaining a native resource.
        COLORREF(unsafe { GetSysColor(index) })
    }
    fn TxGetBackStyle(&self, style: *mut TXTBACKSTYLE) -> Result<()> {
        write(style, TXTBACK_TRANSPARENT)
    }
    fn TxGetMaxLength(&self, length: *mut u32) -> Result<()> {
        write(length, self.state.style.maximum_length())
    }
    fn TxGetScrollBars(&self, bars: *mut u32) -> Result<()> {
        write(bars, self.state.style.scroll_bars())
    }
    fn TxGetPasswordChar(&self) -> Result<i8> {
        Ok(0)
    }
    fn TxGetAcceleratorPos(&self, position: *mut i32) -> Result<()> {
        write(position, -1)
    }
    fn TxGetExtent(&self, size: *mut SIZE) -> Result<()> {
        let bounds = self.state.bounds.get();
        write(size, SIZE { cx: bounds.right * 2540 / 96, cy: bounds.bottom * 2540 / 96 })
    }
    fn OnTxCharFormatChange(&self, format: *const CHARFORMATW) -> Result<()> {
        if format.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // The engine owns format mutations; the host's stable initial defaults stay unchanged.
        self.state.invalidate();
        Ok(())
    }
    fn OnTxParaFormatChange(&self, format: *const PARAFORMAT) -> Result<()> {
        if format.is_null() {
            return Err(E_INVALIDARG.into());
        }
        self.state.invalidate();
        Ok(())
    }
    fn TxGetPropertyBits(&self, mask: u32, bits: *mut u32) -> Result<()> {
        write(bits, mask & self.state.style.properties())
    }
    fn TxNotify(&self, notification: u32, _payload: *mut std::ffi::c_void) -> Result<()> {
        if notification == EN_CHANGE {
            self.state.revision.set(self.state.revision.get().checked_add(1).ok_or(E_INVALIDARG)?);
            if self.state.style == TextStyle::Field && !self.state.closing.get() {
                // SAFETY: The affine child and parent are retained by the native control owner.
                let parent = unsafe { GetParent(self.state.hwnd) }?;
                // SAFETY: Query this child's original scalar identity; no native state changes.
                let id = u16::try_from(unsafe { GetDlgCtrlID(self.state.hwnd) })
                    .map_err(|_| windows::core::Error::from(E_INVALIDARG))?;
                // SAFETY: EN_CHANGE carries only the retained child identity. The parent queues
                // a typed event; it cannot read or mutate this reentrant text document.
                unsafe {
                    SendMessageW(
                        parent,
                        WM_COMMAND,
                        Some(WPARAM(usize::from(id) | ((EN_CHANGE as usize) << 16))),
                        Some(LPARAM(self.state.hwnd.0 as isize)),
                    )
                };
            }
        }
        match notification {
            EN_CHANGE | EN_UPDATE | EN_SELCHANGE | EN_REQUESTRESIZE | EN_HSCROLL | EN_VSCROLL
            | EN_MAXTEXT | EN_SETFOCUS | EN_KILLFOCUS => {
                self.state.invalidate();
                Ok(())
            }
            _ => {
                tracing::error!(
                    operation = "console_native_notification",
                    notification,
                    "Unsupported native text notification"
                );
                Err(E_NOTIMPL.into())
            }
        }
    }
    fn TxImmGetContext(&self) -> HIMC {
        // SAFETY: Borrow the original affine child's current input context.
        unsafe { ImmGetContext(self.state.hwnd) }
    }
    fn TxImmReleaseContext(&self, context: HIMC) {
        // SAFETY: Return only the input context borrowed for this same affine child.
        if let Err(cause) = unsafe { ImmReleaseContext(self.state.hwnd, context) }.ok() {
            self.state.fail("console_host_ime_release", cause);
        }
    }
    fn TxGetSelectionBarWidth(&self, width: *mut i32) -> Result<()> {
        write(width, 0)
    }
}
/// Copy one documented scalar/structure into a checked synchronous native output.
fn write<T>(destination: *mut T, value: T) -> Result<()> {
    if destination.is_null() {
        return Err(E_INVALIDARG.into());
    }
    // SAFETY: Every caller supplies the documented writable native callback output type.
    unsafe {
        destination.write(value);
    }
    Ok(())
}
