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
use windows::{
    Win32::{
        Foundation::{FreeLibrary, RECT},
        System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

#[test]
fn native_windowless_patterns_keep_unicode_and_revoke_escaped_ranges()
-> Result<(), Box<dyn std::error::Error>> {
    let mut apartment = UiApartment::enter()?;
    // SAFETY: The fixture loads only the system module and keeps it through all COM retirement.
    let module = unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }?;
    // SAFETY: This hidden, test-thread-owned STATIC has no visible style and pumps no user input.
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
    let engine = super::super::engine::Engine::new(hwnd);
    engine.initialize(module)?;
    let services = engine.services()?;
    let painting = Rc::new(Cell::new(false));
    let released = Arc::new(AtomicU64::new(0));
    let lease =
        MemoryLease::new(NATIVE_CONSOLE_ACCESSIBILITY_BYTES, Box::new(Release(released.clone())));
    let claim = lease.claim(NATIVE_CONSOLE_ACCESSIBILITY_BYTES)?;
    drop(lease);
    let mut access =
        Accessibility::new(hwnd, &services, module, painting.clone(), "Comfy output", claim)?;
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
        let root = access.root.as_ref().ok_or("root absent")?;
        assert!(root.cast::<windows::Win32::System::Com::IAgileObject>().is_err());
        let fragment: IRawElementProviderFragment = root.cast()?;
        // SAFETY: A real fragment navigation returns the sole actual text child.
        let child = unsafe { fragment.Navigate(NavigateDirection_FirstChild) }?;
        let child: IRawElementProviderSimple = child.cast()?;
        // SAFETY: The child retains guarded native patterns and properties from the sole document.
        let value: IValueProvider =
            unsafe { child.GetPatternProvider(UIA_ValuePatternId) }?.cast()?;
        // SAFETY: Native Value owns the actual read-only state and Unicode text.
        assert!(unsafe { value.IsReadOnly() }?.as_bool());
        // SAFETY: The native result is an owned Unicode string.
        assert!(unsafe { value.Value() }?.to_string().contains(source));
        // SAFETY: Native Text2/TextEdit are retained through guarded wrapper identities.
        let text: ITextProvider2 =
            unsafe { child.GetPatternProvider(UIA_TextPattern2Id) }?.cast()?;
        // SAFETY: The supported readonly native text engine also exposes TextEdit.
        let _: ITextEditProvider =
            unsafe { child.GetPatternProvider(UIA_TextEditPatternId) }?.cast()?;
        // SAFETY: The native document range and clone retain exactly this original text document.
        let range = unsafe { text.DocumentRange() }?;
        assert!(range.cast::<windows::Win32::System::Com::IAgileObject>().is_err());
        // SAFETY: The clone operation is recursively guarded rather than exposing native COM.
        let clone = unsafe { range.Clone() }?;
        // SAFETY: Native shaping/Unicode ownership is observed through the retained range.
        assert!(unsafe { range.GetText(-1) }?.to_string().contains(source));
        // SAFETY: Same-document original ranges are unwrapped only internally.
        assert!(unsafe { range.Compare(&clone) }?.as_bool());
        // SAFETY: A range must return the guarded text fragment rather than the raw native provider.
        let enclosing = unsafe { range.GetEnclosingElement() }?;
        assert_eq!(enclosing.cast::<IUnknown>()?.as_raw(), child.cast::<IUnknown>()?.as_raw());
        // SAFETY: Native selection remains authoritative; array interfaces are guarded recursively.
        unsafe { range.Select() }?;
        // SAFETY: This array transfers to the fixture and is retired after acquiring its first range.
        let selected = unsafe { text.GetSelection() }?;
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
        // SAFETY: The selected range observes the same Unicode document.
        assert!(unsafe { selected_range.GetText(-1) }?.to_string().contains(source));
        let mut rect = RECT::default();
        // SAFETY: The fixture's exact hidden HWND provides independent native physical bounds.
        unsafe { GetWindowRect(hwnd, &mut rect) }?;
        // SAFETY: The fragment reports screen coordinates from that same child owner.
        let actual = unsafe { fragment.BoundingRectangle() }?;
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
        // SAFETY: A range acquired from the selection array resumed through the same guard.
        assert!(unsafe { selected_range.GetText(-1) }?.to_string().contains(source));
        // SAFETY: The guard resumes the same unchanged document rather than replacing its engine.
        assert!(unsafe { clone.GetText(-1) }?.to_string().contains(source));
        let _publication = access.message(WPARAM(0), LPARAM(UiaRootObjectId as isize))?;
        let mut node = HUIANODE::default();
        // SAFETY: This direct UIA client registration owns one node for the exact retained root.
        unsafe { UiaNodeFromProvider(access.root.as_ref().ok_or("root retired")?, &mut node) }?;
        access.published.set(true);
        access.close()?;
        assert_eq!(released.load(Ordering::Acquire), 0);
        // SAFETY: The fixture's UIA client node is released after provider disconnection.
        if !unsafe { UiaNodeRelease(node) }.as_bool() {
            return Err("UIA client node release failed".into());
        }
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
    access.close()?;
    drop(access);
    drop(services);
    engine.close()?;
    drop(engine);
    // SAFETY: Every native provider/document/host has retired before the owned child and module.
    unsafe { DestroyWindow(hwnd) }?;
    // SAFETY: No surviving COM wrapper retains any native interface into the system module.
    unsafe { FreeLibrary(module) }?;
    // SAFETY: This fixture has completed all exact native retirement obligations.
    unsafe { apartment.release_after_teardown() }?;
    result
}
