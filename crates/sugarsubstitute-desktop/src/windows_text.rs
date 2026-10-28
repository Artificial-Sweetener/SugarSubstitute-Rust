// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one system RichEdit document behind an affine native input window.
use crate::DesktopError;
use sugarsubstitute_ui::windows::text_style::TextStyle;
#[path = "windows_text/accessibility.rs"]
mod accessibility;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetProcAddress,
        UI::Controls::RichEdit::{
            EM_GETEDITSTYLEEX, EM_SETEDITSTYLEEX, ITextServices2, SES_EX_NOACETATESELECTION,
        },
    },
    core::{GUID, IUnknown, Interface},
};
#[path = "windows_text/host.rs"]
mod host;
pub(crate) use host::State;
#[path = "windows_text/host_interface.rs"]
mod host_interface;
#[path = "windows_text/input.rs"]
mod input;
pub(crate) use input::find;
#[path = "windows_text/field.rs"]
mod field;
pub(crate) mod library;
#[path = "windows_text/renderer.rs"]
mod renderer;
#[path = "windows_text/retirement.rs"]
mod retirement;
#[path = "windows_text/scroll.rs"]
pub(crate) mod scroll;
#[path = "windows_text/scroll_adapter.rs"]
mod scroll_adapter;

/// The module and child remain retained outside this owner until checked shutdown succeeds.
pub(crate) struct Engine {
    services: RefCell<Option<ITextServices2>>,
    host: RefCell<Option<host_interface::HostInterface>>,
    pub(crate) state: Rc<State>,
    closed: Cell<bool>,
    module: Cell<Option<HMODULE>>,
    shutdown: Cell<bool>,
    input: RefCell<Option<input::Input>>,
    accessibility: RefCell<Option<accessibility::Accessibility>>,
    accessibility_label: Cell<Option<&'static str>>,
    renderer: RefCell<Option<renderer::Renderer>>,
}
impl Engine {
    /// Construct inert retained custody before any factory or subclass effect.
    pub(crate) fn new(hwnd: HWND, style: TextStyle) -> Rc<Self> {
        let state = Rc::new(State::new(hwnd, style));
        let host = host_interface::create(state.clone());
        Rc::new(Self {
            services: RefCell::new(None),
            host: RefCell::new(Some(host)),
            state,
            closed: Cell::new(false),
            module: Cell::new(None),
            shutdown: Cell::new(false),
            input: RefCell::new(None),
            accessibility: RefCell::new(None),
            accessibility_label: Cell::new(None),
            renderer: RefCell::new(None),
        })
    }
    /// Initialize only after the containing native owner adopted this exact partial lifetime.
    pub(crate) fn initialize(&self, module: HMODULE) -> Result<(), DesktopError> {
        if self.services.borrow().is_some() || self.closed.get() {
            return Err(DesktopError::message(
                "console_factory",
                "document already initialized or retired",
            ));
        }
        self.module.set(Some(module));
        self.refresh_system_theme()?;
        // SAFETY: The retained system DLL exports a synchronous documented COM factory.
        let factory = unsafe { GetProcAddress(module, windows::core::s!("CreateTextServices")) }
            .ok_or_else(|| {
                DesktopError::message("console_factory", "system text-services factory absent")
            })?;
        type Factory = unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *mut *mut c_void,
        ) -> windows::core::HRESULT;
        // SAFETY: The exported system ABI has exactly the CreateTextServices signature.
        let factory: Factory = unsafe { std::mem::transmute(factory) };
        let host = self.host.borrow();
        let interface = host
            .as_ref()
            .ok_or_else(|| DesktopError::message("console_host", "native host absent"))?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: This affine host and output storage remain owned across all factory callbacks.
        unsafe { factory(std::ptr::null_mut(), interface.as_raw(), &mut raw) }
            .ok()
            .map_err(|cause| DesktopError::new("console_factory", cause))?;
        if raw.is_null() {
            return Err(DesktopError::message(
                "console_factory",
                "system factory returned no document",
            ));
        }
        // SAFETY: Successful factory output transfers one owned IUnknown reference.
        let unknown = unsafe { IUnknown::from_raw(raw) };
        let services_iid = exported_iid(module, windows::core::s!("IID_ITextServices2"))?;
        let mut raw_services = std::ptr::null_mut();
        // SAFETY: Query the DLL's real exported IID; generated metadata uses GUID zero here.
        unsafe {
            (unknown.vtable().QueryInterface)(unknown.as_raw(), &services_iid, &mut raw_services)
        }
        .ok()
        .map_err(|cause| DesktopError::new("console_services", cause))?;
        if raw_services.is_null() {
            return Err(DesktopError::message(
                "console_services",
                "system document interface absent",
            ));
        }
        // SAFETY: The checked DLL IID guarantees the documented ITextServices2 vtable.
        *self.services.borrow_mut() = Some(unsafe { ITextServices2::from_raw(raw_services) });
        drop(host);
        let exported_host = exported_iid(module, windows::core::s!("IID_ITextHost"))?;
        if exported_host != host_interface::HostInterface::IID {
            return Err(DesktopError::message(
                "console_host",
                "system host interface identity differs",
            ));
        }
        // The presentation owns selection RGBA. Native acetate selection preblends its
        // background token, so request the documented exact system-color selection path.
        let selection = SES_EX_NOACETATESELECTION;
        let applied = self.message(
            EM_SETEDITSTYLEEX,
            WPARAM(selection as usize),
            LPARAM(selection as isize),
        )?;
        let observed = self.message(EM_GETEDITSTYLEEX, WPARAM(0), LPARAM(0))?;
        if applied.0 & selection as isize == 0 || observed.0 & selection as isize == 0 {
            return Err(DesktopError::message(
                "console_selection_style",
                "native exact-color selection policy rejected",
            ));
        }
        // The shared role owns whether the document admits bounded edit history.
        self.message(
            windows::Win32::UI::Controls::RichEdit::EM_SETUNDOLIMIT,
            WPARAM(self.state.style.undo_limit()),
            LPARAM(0),
        )?;
        if self.state.style == TextStyle::Field {
            use windows::Win32::UI::Controls::RichEdit::{
                EM_SETEVENTMASK, ENM_CHANGE, ENM_SELCHANGE,
            };
            self.message(
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
            )?;
        }
        Ok(())
    }
    /// Borrow the original document without exporting native scheduler or HWND privileges.
    pub(crate) fn services(&self) -> Result<ITextServices2, DesktopError> {
        self.services
            .borrow()
            .as_ref()
            .cloned()
            .ok_or_else(|| DesktopError::message("console_services", "document retired"))
    }
    /// Forward one affine input/document message to the same engine used for drawing.
    pub(crate) fn message(
        &self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<LRESULT, DesktopError> {
        if self.closed.get() || self.state.closing.get() {
            return Err(DesktopError::message("console_message", "document closing"));
        }
        self.forward(message, wparam, lparam)?.ok_or_else(|| {
            DesktopError::message("console_message", "native text operation was not processed")
        })
    }
    /// Preserve S_FALSE so actual unprocessed window messages continue through the native chain.
    fn forward(
        &self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<Option<LRESULT>, DesktopError> {
        self.state.check()?;
        let services = self.services()?;
        let mut result = LRESULT(0);
        // SAFETY: The documented synchronous message buffers and affine engine remain retained.
        let status = unsafe {
            (services.vtable().base__.TxSendMessage)(
                services.as_raw(),
                message,
                wparam,
                lparam,
                &mut result,
            )
        };
        status.ok().map_err(|cause| DesktopError::new("console_message", cause))?;
        self.state.check()?;
        Ok((status != windows::Win32::Foundation::S_FALSE).then_some(result))
    }
    /// Publish actual child geometry before enabling the native layout and selection owner.
    pub(crate) fn activate(&self) -> Result<(), DesktopError> {
        self.state.refresh_bounds()?;
        let mut bounds = self.state.client_bounds();
        // SAFETY: The retained system document and exact affine child share this current thread.
        unsafe { self.services()?.OnTxInPlaceActivate(&mut bounds) }
            .map_err(|cause| DesktopError::new("console_activate", cause))?;
        self.state.check()
    }
    /// Surface callback failures before the outer native transaction can accept any effect.
    pub(crate) fn check(&self) -> Result<(), DesktopError> {
        if self.closed.get() || self.state.closing.get() {
            return Err(DesktopError::message("console_engine", "document closing or retired"));
        }
        self.state.check()
    }
    /// Retain localized provider configuration while the original child remains hidden.
    /// Actual provider allocation requires the first admitted frame in prepare().
    pub(crate) fn configure_accessibility(
        &self,
        module: HMODULE,
        label: &'static str,
    ) -> Result<(), DesktopError> {
        if self.module.get() != Some(module)
            || self.accessibility_label.get().is_some()
            || label.is_empty()
        {
            return Err(DesktopError::message(
                "console_accessibility",
                "provider configuration already present or module differs",
            ));
        }
        self.accessibility_label.set(Some(label));
        Ok(())
    }
    /// Rename the original native accessible document while preserving focus, edits and ranges.
    pub(crate) fn rename(&self, module: HMODULE, label: &'static str) -> Result<(), DesktopError> {
        self.check()?;
        if self.module.get() != Some(module) || label.is_empty() || label.contains('\0') {
            return Err(DesktopError::message(
                "text_accessibility_name",
                "invalid label or module",
            ));
        }
        if let Some(owner) = self.accessibility.borrow().as_ref() {
            owner.rename(label)?;
        }
        self.accessibility_label.set(Some(label));
        Ok(())
    }
    /// Claim original-frame metadata before allocating any externally retained provider wrapper.
    fn admit_accessibility(
        &self,
        frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
    ) -> Result<(), DesktopError> {
        if let Some(owner) = self.accessibility.borrow().as_ref() {
            return owner.fund(frame);
        }
        let label = self.accessibility_label.get().ok_or_else(|| {
            DesktopError::message("console_accessibility", "provider configuration absent")
        })?;
        let module = self.module.get().ok_or_else(|| {
            DesktopError::message("console_accessibility", "native module absent")
        })?;
        let claim = frame
            .claim_storage(
                sugarsubstitute_contracts::presentation::NATIVE_CONSOLE_ACCESSIBILITY_BYTES,
            )
            .map_err(|cause| DesktopError::new("console_accessibility_claim", cause))?;
        *self.accessibility.borrow_mut() = Some(accessibility::Accessibility::new(
            self.state.hwnd,
            &self.services()?,
            module,
            self.state.observing_blocked.clone(),
            label,
            claim,
        )?);
        Ok(())
    }
    /// Preserve nested redraw scopes without changing native child availability.
    pub(crate) fn redraw_allowed(&self) -> bool {
        self.state.redraw.get()
    }
    /// Expose checked completed retirement to the containing creation transaction.
    pub(crate) fn retired(&self) -> bool {
        self.closed.get()
    }
    /// Retain the admitted background for subsequent WM_PAINT. Transcripts also require their
    /// immutable semantic index; editable fields paint their authoritative native document directly.
    pub(crate) fn prepare(
        &self,
        frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
        document: Option<Rc<crate::console::index::DocumentIndex>>,
        bounds: windows::Win32::Foundation::RECT,
        dark: bool,
        scale: f64,
    ) -> Result<(), DesktopError> {
        self.state.check()?;
        if document.is_some() != (self.state.style == TextStyle::Transcript) {
            return Err(DesktopError::message(
                "text_paint_role",
                "semantic transcript context differs from the native document role",
            ));
        }
        if !scale.is_finite() || scale <= 0.0 {
            return Err(DesktopError::message("console_scale", "invalid native scale"));
        }
        let scale_changed = self.state.scale.replace(scale) != scale;
        self.admit_accessibility(frame)?;
        if self.renderer.borrow().is_none() {
            *self.renderer.borrow_mut() =
                Some(renderer::Renderer::new(frame, document.clone(), bounds, dark)?);
        }
        let changed = self
            .renderer
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| DesktopError::message("console_renderer", "paint custody absent"))?
            .prepare(frame, document, bounds, dark)?;
        if changed || scale_changed {
            self.state.invalidate();
        } else {
            self.state.expose();
        }
        self.state.check()
    }
    /// Retain the hidden field's document and providers while retiring its obsolete painted frame.
    pub(crate) fn hide_presentation(
        &self,
        frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
    ) -> Result<(), DesktopError> {
        self.check()?;
        if let Some(owner) = self.accessibility.borrow().as_ref() {
            owner.fund(frame)?;
        }
        let mut renderer = self.renderer.borrow_mut();
        if let Some(owner) = renderer.as_mut() {
            owner.close()?;
        }
        renderer.take();
        self.state.invalidate();
        Ok(())
    }

    /// Observe native scroll state from the authoritative text engine without a stock scrollbar HWND.
    pub(crate) fn at_bottom(&self) -> Result<bool, DesktopError> {
        Ok(scroll::Scroll::observe(&self.services()?)?.at_bottom())
    }
    #[cfg(any(test, feature = "boundary-fixtures"))]
    /// Borrow current native pixels without rebuilding an application-owned text approximation.
    pub(crate) fn capture(
        &self,
        visit: &mut dyn FnMut(windows::Win32::Foundation::RECT, usize, &[u8]),
    ) -> Result<(), DesktopError> {
        self.renderer
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| DesktopError::message("native_text_capture", "Renderer absent"))?
            .capture(self, visit)
    }
    #[cfg(test)]
    /// Count actual RichEdit raster transactions independently of expose/composite requests.
    pub(crate) fn qualified_glyph_draws(&self) -> usize {
        self.renderer.borrow().as_ref().map_or(0, |renderer| renderer.glyph_draws)
    }
    #[cfg(test)]
    /// Paint the actual native engine into a separately admitted fixture DIB without showing a HWND.
    pub(crate) fn qualify_pixels(
        &self,
        dc: windows::Win32::Graphics::Gdi::HDC,
        alpha: bool,
        bytes: &mut [u8],
    ) -> Result<(), DesktopError> {
        let mut renderer = self.renderer.borrow_mut();
        let renderer = renderer
            .as_mut()
            .ok_or_else(|| DesktopError::message("console_qualify", "renderer absent"))?;
        renderer.paint(self, dc)?;
        if alpha {
            renderer.read_glyphs(bytes)?;
        }
        Ok(())
    }
    #[cfg(test)]
    /// Observe the existing native alpha allocation without issuing another paint transaction.
    pub(crate) fn qualified_alpha(&self, bytes: &mut [u8]) -> Result<(), DesktopError> {
        self.renderer
            .borrow()
            .as_ref()
            .ok_or_else(|| DesktopError::message("console_qualify", "renderer absent"))?
            .read_glyphs(bytes)
    }
    /// Drain native timers and document references before its HWND and system DLL may retire.
    pub(crate) fn close(&self) -> Result<(), DesktopError> {
        if self.closed.get() {
            return Ok(());
        }
        self.state.closing.set(true);
        if let Some(accessibility) = self.accessibility.borrow_mut().as_mut() {
            accessibility.close()?;
        }
        self.accessibility.borrow_mut().take();
        if let Some(input) = self.input.borrow().as_ref() {
            input.close()?;
        }
        self.input.borrow_mut().take();
        self.state.release_capture()?;
        self.state.stop_timers()?;
        self.shutdown_document()?;
        self.state.caret.close()?;
        self.services.borrow_mut().take();
        if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
            renderer.close()?;
        }
        self.renderer.borrow_mut().take();
        self.host.borrow_mut().take();
        self.state.painting.set(false);
        self.state.retired()?;
        self.closed.set(true);
        self.state.check()
    }
}
/// Read the identity bytes from the same retained system module that implements the interface.
pub(crate) fn exported_iid(
    module: HMODULE,
    name: windows::core::PCSTR,
) -> Result<GUID, DesktopError> {
    // SAFETY: Documented exported IID data remains valid until the containing library retires.
    let symbol = unsafe { GetProcAddress(module, name) }.ok_or_else(|| {
        DesktopError::message("console_interface_identity", "system interface export absent")
    })?;
    // SAFETY: This named symbol is a GUID data export rather than an executable entry point.
    Ok(unsafe { std::ptr::read_unaligned(symbol as *const GUID) })
}

#[cfg(test)]
#[path = "windows_text/field_tests.rs"]
mod field_tests;
