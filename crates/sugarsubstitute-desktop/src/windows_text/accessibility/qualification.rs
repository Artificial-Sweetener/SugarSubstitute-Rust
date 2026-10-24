// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real system RichEdit patterns and retained ranges without capture or visible windows.
use super::*;
use crate::windows_ui_apartment::UiApartment;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use sugarsubstitute_ui::windows::text_style::TextStyle;
use windows::{
    Win32::{
        Foundation::{FreeLibrary, RECT},
        System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        UI::WindowsAndMessaging::*,
    },
    core::{PCWSTR, w},
};

#[path = "client_qualification.rs"]
mod client;

struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

/// Both native roles must preserve the same guarded document and retirement semantics.
pub(crate) fn qualify_patterns(
    style: TextStyle,
    client: impl FnOnce(&crate::windows_text::Engine, &str) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut apartment = UiApartment::enter()?;
    // SAFETY: The fixture loads only the system module and keeps it through all COM retirement.
    let module = unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }?;
    // SAFETY: This owned hidden fixture never receives user input.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(""),
            WS_POPUP,
            10,
            20,
            300,
            120,
            None,
            None,
            None,
            None,
        )
    }?;
    let parent = hwnd;
    // SAFETY: The hidden native parent owns this child, matching the actual composed text control.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(""),
            WS_CHILD,
            10,
            20,
            280,
            90,
            Some(parent),
            None,
            None,
            None,
        )
    }?;
    let engine = crate::windows_text::Engine::new(hwnd, style);
    engine.initialize(module)?;
    let services = engine.services()?;
    let painting = Rc::new(Cell::new(false));
    let released = Arc::new(AtomicU64::new(0));
    let lease =
        MemoryLease::new(NATIVE_CONSOLE_ACCESSIBILITY_BYTES, Box::new(Release(released.clone())));
    let claim = lease.claim(NATIVE_CONSOLE_ACCESSIBILITY_BYTES)?;
    drop(lease);
    *engine.accessibility.borrow_mut() =
        Some(Accessibility::new(hwnd, &services, module, painting.clone(), "Comfy output", claim)?);
    engine.install_input()?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let source = "Unicode 😀 e\u{301} שלום";
        let text: Vec<u16> = source.encode_utf16().chain([0]).collect();
        let mut written = LRESULT(0);
        // SAFETY: The synchronous native message borrows this owned terminated Unicode buffer.
        unsafe {
            services.TxSendMessage(
                WM_SETTEXT,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
                &mut written,
            )
        }?;
        engine.activate()?;
        let root = engine
            .accessibility
            .borrow()
            .as_ref()
            .and_then(|access| access.root.clone())
            .ok_or("root absent")?;
        assert!(root.cast::<windows::Win32::System::Com::IAgileObject>().is_err());
        let fragment: IRawElementProviderFragment = root.cast()?;
        // SAFETY: A real fragment navigation returns the sole actual text child.
        let child = unsafe { fragment.Navigate(NavigateDirection_FirstChild) }
            .map_err(|cause| DesktopError::new("text_qualify_navigate", cause))?;
        let child: IRawElementProviderSimple = child.cast()?;
        engine.rename(module, "保存先")?;
        for provider in [&root, &child] {
            // SAFETY: Both retained providers belong to this same live affine document.
            let name = unsafe { provider.GetPropertyValue(UIA_NamePropertyId) }?;
            assert_eq!(BSTR::try_from(&name)?.to_string(), "保存先");
        }
        // SAFETY: The child retains guarded native patterns and properties from the sole document.
        let value: IValueProvider = unsafe { child.GetPatternProvider(UIA_ValuePatternId) }
            .map_err(|cause| DesktopError::new("text_qualify_value_pattern", cause))?
            .cast()?;
        assert_eq!(
            // SAFETY: Native Value owns the actual read-only state and Unicode text.
            unsafe { value.IsReadOnly() }
                .map_err(|cause| DesktopError::new("text_qualify_readonly", cause))?
                .as_bool(),
            style == TextStyle::Transcript
        );
        assert!(
            // SAFETY: The native result is an owned Unicode string.
            unsafe { value.Value() }
                .map_err(|cause| DesktopError::new("text_qualify_value", cause))?
                .to_string()
                .contains(source)
        );
        if style == TextStyle::Field {
            exercise_field(&engine, &value, source)?;
        }
        // SAFETY: Native Text2/TextEdit are retained through guarded wrapper identities.
        let text: ITextProvider2 = unsafe { child.GetPatternProvider(UIA_TextPattern2Id) }
            .map_err(|cause| DesktopError::new("text_qualify_text_pattern", cause))?
            .cast()?;
        // SAFETY: The supported readonly native text engine also exposes TextEdit.
        let _: ITextEditProvider = unsafe { child.GetPatternProvider(UIA_TextEditPatternId) }
            .map_err(|cause| DesktopError::new("text_qualify_edit_pattern", cause))?
            .cast()?;
        // SAFETY: The native document range and clone retain exactly this original text document.
        let range = unsafe { text.DocumentRange() }
            .map_err(|cause| DesktopError::new("text_qualify_document_range", cause))?;
        assert!(range.cast::<windows::Win32::System::Com::IAgileObject>().is_err());
        // SAFETY: The clone operation is recursively guarded rather than exposing native COM.
        let clone = unsafe { range.Clone() }
            .map_err(|cause| DesktopError::new("text_qualify_range_clone", cause))?;
        assert!(
            // SAFETY: Native shaping/Unicode ownership is observed through the retained range.
            unsafe { range.GetText(-1) }
                .map_err(|cause| DesktopError::new("text_qualify_range_text", cause))?
                .to_string()
                .contains(source)
        );
        assert!(
            // SAFETY: Same-document original ranges are unwrapped only internally.
            unsafe { range.Compare(&clone) }
                .map_err(|cause| DesktopError::new("text_qualify_range_compare", cause))?
                .as_bool()
        );
        // SAFETY: A range must return the guarded text fragment rather than the raw native provider.
        let enclosing = unsafe { range.GetEnclosingElement() }
            .map_err(|cause| DesktopError::new("text_qualify_range_enclosing", cause))?;
        assert_eq!(enclosing.cast::<IUnknown>()?.as_raw(), child.cast::<IUnknown>()?.as_raw());
        // SAFETY: Native selection remains authoritative; array interfaces are guarded recursively.
        unsafe { range.Select() }
            .map_err(|cause| DesktopError::new("text_qualify_range_select", cause))?;
        // SAFETY: This array transfers to the fixture and is retired after acquiring its first range.
        let selected = unsafe { text.GetSelection() }
            .map_err(|cause| DesktopError::new("text_qualify_selection", cause))?;
        let mut selected_pointer: *mut c_void = std::ptr::null_mut();
        // SAFETY: A full selected document returns one AddRef'd range at index zero.
        unsafe {
            windows::Win32::System::Ole::SafeArrayGetElement(
                selected,
                &0,
                (&raw mut selected_pointer).cast(),
            )
        }?;
        if selected_pointer.is_null() {
            return Err("selected range absent".into());
        }
        // SAFETY: The successful array read transferred one guarded owned range reference.
        let selected_range = unsafe { ITextRangeProvider::from_raw(selected_pointer) };
        // SAFETY: Array ownership is independent of the acquired range's reference.
        unsafe { windows::Win32::System::Ole::SafeArrayDestroy(selected) }?;
        assert!(
            // SAFETY: The selected range observes the same Unicode document.
            unsafe { selected_range.GetText(-1) }
                .map_err(|cause| DesktopError::new("text_qualify_selected_text", cause))?
                .to_string()
                .contains(source)
        );
        let mut rect = RECT::default();
        // SAFETY: The fixture's exact hidden HWND provides independent native physical bounds.
        unsafe { GetWindowRect(hwnd, &mut rect) }?;
        // SAFETY: The fragment reports screen coordinates from that same child owner.
        let actual = unsafe { fragment.BoundingRectangle() }
            .map_err(|cause| DesktopError::new("text_qualify_bounds", cause))?;
        assert_eq!(
            (actual.left, actual.top, actual.width, actual.height),
            (
                f64::from(rect.left),
                f64::from(rect.top),
                f64::from(rect.right - rect.left),
                f64::from(rect.bottom - rect.top)
            )
        );
        painting.set(true);
        // SAFETY: Previously acquired interfaces must reject even though no new pattern lookup runs.
        let blocked_text = unsafe { range.GetText(-1) };
        assert_eq!(blocked_text.err().ok_or("paint exposed text")?.code(), unavailable().code());
        // SAFETY: Previously acquired clones must share the same gate.
        let blocked_format = unsafe { clone.GetAttributeValue(UIA_ForegroundColorAttributeId) };
        assert_eq!(
            blocked_format.err().ok_or("paint exposed formatting")?.code(),
            unavailable().code()
        );
        // SAFETY: A retained Value pattern is guarded independently of the root provider.
        let blocked_value = unsafe { value.Value() };
        assert_eq!(blocked_value.err().ok_or("paint exposed value")?.code(), unavailable().code());
        // SAFETY: Selection arrays cannot provide a route around the paint guard.
        let blocked_selection = unsafe { selected_range.GetText(-1) };
        assert_eq!(
            blocked_selection.err().ok_or("paint exposed selection")?.code(),
            unavailable().code()
        );
        painting.set(false);
        assert!(
            // SAFETY: A range acquired from the selection array resumed through the same guard.
            unsafe { selected_range.GetText(-1) }
                .map_err(|cause| DesktopError::new("text_qualify_selected_text", cause))?
                .to_string()
                .contains(source)
        );
        assert!(
            // SAFETY: The guard resumes the same unchanged document rather than replacing its engine.
            unsafe { clone.GetText(-1) }
                .map_err(|cause| DesktopError::new("text_qualify_clone_text", cause))?
                .to_string()
                .contains(source)
        );
        // SAFETY: Compare the client to the native document's complete Value, including paragraph terminators.
        let native_value = unsafe { value.Value() }?.to_string();
        client(&engine, &native_value)?;
        assert_eq!(released.load(Ordering::Acquire), 0);
        // SAFETY: Close revokes every native lease even while callers retain wrapper COM objects.
        let retired_text = unsafe { range.GetText(-1) };
        assert_eq!(
            retired_text.err().ok_or("closed range stayed live")?.code(),
            unavailable().code()
        );
        // SAFETY: The retained pattern similarly cannot touch the retired document.
        let retired_value = unsafe { value.Value() };
        assert_eq!(
            retired_value.err().ok_or("closed value stayed live")?.code(),
            unavailable().code()
        );
        Ok(())
    })();
    painting.set(false);
    drop(services);
    engine.close()?;
    drop(engine);
    // SAFETY: Every native provider/document/host has retired before the owned child and module.
    unsafe { DestroyWindow(hwnd) }?;
    // SAFETY: The parent retires only after its text child and callbacks have retired.
    unsafe { DestroyWindow(parent) }?;
    // SAFETY: No surviving COM wrapper retains any native interface into the system module.
    unsafe { FreeLibrary(module) }?;
    // SAFETY: This fixture has completed all exact native retirement obligations.
    unsafe { apartment.release_after_teardown() }?;
    result
}

/// Exercise actual system editing and undo without focus, clipboard writes or synthetic user input.
fn exercise_field(
    engine: &crate::windows_text::Engine,
    value: &IValueProvider,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Controls::{EM_CANUNDO, EM_REPLACESEL, EM_SETSEL, EM_UNDO};
    let revision = engine.state.revision.get();
    engine.message(EM_SETSEL, WPARAM(0), LPARAM(-1))?;
    let replacement: Vec<u16> = "E:\\日本語 😀".encode_utf16().chain([0]).collect();
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(replacement.as_ptr() as isize))?;
    assert_eq!(
        // SAFETY: The guarded value reads this live, affine native document.
        unsafe { value.Value() }
            .map_err(|cause| DesktopError::new("text_qualify_value", cause))?
            .to_string(),
        "E:\\日本語 😀"
    );
    assert!(engine.state.revision.get() > revision);
    assert_ne!(engine.message(EM_CANUNDO, WPARAM(0), LPARAM(0))?.0, 0);
    assert_ne!(engine.message(EM_UNDO, WPARAM(0), LPARAM(0))?.0, 0);
    assert_eq!(
        // SAFETY: Native undo must restore the exact original Unicode document.
        unsafe { value.Value() }
            .map_err(|cause| DesktopError::new("text_qualify_value", cause))?
            .to_string(),
        source
    );
    let limit = TextStyle::Field.maximum_length() as usize;
    let mut maximum: Vec<u16> = "😀".repeat(limit / 2).encode_utf16().chain([0]).collect();
    // SAFETY: The native Value setter borrows this exact terminated UTF-16 input synchronously.
    unsafe { value.SetValue(PCWSTR(maximum.as_ptr())) }?;
    // SAFETY: This live native document owns the returned text.
    let before_rejection = unsafe { value.Value() }?.to_string();
    assert_eq!(before_rejection.encode_utf16().count(), limit);
    maximum.insert(limit, b'X' as u16);
    // SAFETY: The complete oversized input remains a valid terminated UTF-16 string.
    let rejected = unsafe { value.SetValue(PCWSTR(maximum.as_ptr())) };
    assert!(rejected.is_err(), "UIA bypassed the editable field length limit");
    // SAFETY: Rejected edits must leave the same native document untouched.
    assert_eq!(unsafe { value.Value() }?.to_string(), before_rejection);
    let restored: Vec<u16> = source.encode_utf16().chain([0]).collect();
    // SAFETY: Restore the exact initial document for the retained range/client qualification.
    unsafe { value.SetValue(PCWSTR(restored.as_ptr())) }?;
    assert!(!engine.state.scroll_visible.get());
    engine.check()?;
    Ok(())
}

/// Exercise a real external-to-STA UIA client within a dedicated native qualification process.
pub(crate) fn qualify_client(style: TextStyle) -> Result<(), Box<dyn std::error::Error>> {
    qualify_patterns(style, |engine, source| {
        client::qualify(engine, source, style == TextStyle::Transcript, None)
    })
}

/// A second live field shares the same native STA but must keep its own UIA and caret identity.
pub(crate) fn qualify_sibling_retirement(
    first: &crate::windows_text::Engine,
    second: &crate::windows_text::Engine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = BSTR::new();
    let mut sibling = BSTR::new();
    // SAFETY: Both affine native documents write independently owned BSTR values synchronously.
    unsafe {
        first.services()?.TxGetText(&mut source)?;
        second.services()?.TxGetText(&mut sibling)?;
    }
    client::qualify(first, &source.to_string(), false, Some((second, sibling.to_string())))
}
