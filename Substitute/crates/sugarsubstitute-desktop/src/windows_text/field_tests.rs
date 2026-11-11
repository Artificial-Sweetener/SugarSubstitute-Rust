// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual editable focus and caret retirement on an owned inactive desktop.
use super::Engine;
use crate::windows_ui_apartment::UiApartment;
use std::rc::Rc;
use sugarsubstitute_ui::windows::text_style::TextStyle;
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND, LPARAM, WPARAM},
        System::{
            LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
            Threading::GetCurrentThreadId,
        },
        UI::{
            Input::KeyboardAndMouse::{GetActiveWindow, GetFocus, SetActiveWindow, SetFocus},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};
#[path = "field_render_tests.rs"]
mod rendering;
#[path = "field_value_tests.rs"]
mod values;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Retain partial native ownership even if a later construction or retirement operation rejects.
#[derive(Default)]
struct Fields {
    windows: Vec<HWND>,
    engines: Vec<Rc<Engine>>,
    module: Option<HMODULE>,
}
impl Fields {
    fn open(&mut self) -> TestResult {
        // SAFETY: Only the OS-provided text implementation is admitted and retained through closure.
        self.module = Some(unsafe {
            LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        }?);
        // SAFETY: The current test thread is attached to its private inactive desktop.
        let parent = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                0,
                0,
                300,
                100,
                None,
                None,
                None,
                None,
            )
        }?;
        self.windows.push(parent);
        for row in 0..2 {
            // SAFETY: This exact retained parent owns both children on the same private desktop.
            let child = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    row * 40,
                    250,
                    32,
                    Some(parent),
                    None,
                    None,
                    None,
                )
            }?;
            self.windows.push(child);
            let engine = Engine::new(child, TextStyle::Field);
            self.engines.push(engine.clone());
            engine.initialize(self.module.ok_or("text module absent")?)?;
            engine.configure_accessibility(self.module.ok_or("text module absent")?, "Path")?;
            engine.install_input()?;
            engine.activate()?;
            engine.message(WM_SETTEXT, WPARAM(0), LPARAM(w!("Editable path").as_ptr() as isize))?;
        }
        // SAFETY: This window belongs to the private desktop; no switch rights were acquired.
        unsafe {
            let _previous = ShowWindow(parent, SW_SHOWNOACTIVATE);
            let _previous = SetActiveWindow(parent);
            assert_eq!(GetActiveWindow(), parent);
        }
        Ok(())
    }
    fn close(&mut self) -> TestResult {
        while let Some(engine) = self.engines.last() {
            engine.close()?;
            self.engines.pop();
        }
        while let Some(hwnd) = self.windows.last().copied() {
            // SAFETY: Children retire before the parent after their callback/document owners close.
            unsafe { DestroyWindow(hwnd) }?;
            self.windows.pop();
        }
        if let Some(module) = self.module {
            // SAFETY: Every native document and callback into the exact module has retired.
            unsafe { FreeLibrary(module) }?;
            self.module = None;
        }
        Ok(())
    }
}
impl Drop for Fields {
    fn drop(&mut self) {
        if !self.windows.is_empty() || self.module.is_some() {
            tracing::error!(
                operation = "text_field_fixture_retained",
                "Native field fixture did not retire"
            );
        }
    }
}

fn caret() -> Result<GUITHREADINFO, Box<dyn std::error::Error>> {
    let mut info =
        GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    // SAFETY: Observe this test thread's queue, never the interactive desktop's thread.
    unsafe { GetGUIThreadInfo(GetCurrentThreadId(), &mut info) }?;
    Ok(info)
}

#[test]
fn native_field_focus_moves_caret_and_old_field_close_preserves_new_owner() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_field_focus_moves_caret_and_old_field_close_preserves_new_owner"
        ),
        qualify,
    )
}

fn qualify() -> TestResult {
    let mut apartment = UiApartment::enter()?;
    let mut fields = Fields::default();
    let result = (|| -> TestResult {
        fields.open()?;
        let first = fields.engines.first().ok_or("first field absent")?;
        let second = fields.engines.get(1).ok_or("second field absent")?;
        for engine in [first, second, first, second] {
            // SAFETY: Focus changes are limited to the two retained private-desktop children.
            unsafe {
                let _previous = SetFocus(Some(engine.state.hwnd));
            }
            // SAFETY: The native focus query observes only this current test thread.
            assert_eq!(unsafe { GetFocus() }, engine.state.hwnd);
            engine.check()?;
            let observed = caret()?;
            assert_eq!(observed.hwndCaret, engine.state.hwnd);
            assert!(observed.flags.contains(GUI_CARETBLINKING));
            assert!(engine.state.caret_visible.get());
        }
        second.state.caret.show(false)?;
        second.state.caret.show(false)?;
        assert!(!caret()?.flags.contains(GUI_CARETBLINKING));
        second.state.caret.show(true)?;
        second.state.caret.show(true)?;
        assert!(caret()?.flags.contains(GUI_CARETBLINKING));
        rendering::qualify(first)?;
        rendering::qualify(second)?;
        super::accessibility::qualification::qualify_sibling_retirement(first, second)?;
        exercise_long_field(second)?;
        assert_eq!(caret()?.hwndCaret, second.state.hwnd);
        second.check()?;
        second.close()?;
        assert!(caret()?.hwndCaret.is_invalid());
        Ok(())
    })();
    fields.close()?;
    // SAFETY: All fixture windows, text documents and callbacks have completed checked retirement.
    unsafe { apartment.release_after_teardown() }?;
    result
}

/// Count routed events whose operation reports an unsupported native text notification.
struct UnsupportedNotifications(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl tracing::Subscriber for UnsupportedNotifications {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Operation(bool);
        impl tracing::field::Visit for Operation {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0 |= field.name() == "operation" && value == "console_native_notification";
            }
            fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
        }
        let mut operation = Operation(false);
        event.record(&mut operation);
        if operation.0 {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// Gaining and losing focus are ordinary text notifications, never reported as unsupported.
#[test]
fn native_field_focus_changes_are_supported_text_notifications() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::native_field_focus_changes_are_supported_text_notifications"),
        || {
            let mut apartment = UiApartment::enter()?;
            let mut fields = Fields::default();
            let unsupported = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let dispatch = tracing::Dispatch::new(UnsupportedNotifications(unsupported.clone()));
            let result = tracing::dispatcher::with_default(&dispatch, || -> TestResult {
                fields.open()?;
                let first = fields.engines.first().ok_or("first field absent")?;
                let second = fields.engines.get(1).ok_or("second field absent")?;
                for engine in [first, second, first] {
                    // SAFETY: Focus moves only between the two retained private-desktop children.
                    unsafe {
                        let _previous = SetFocus(Some(engine.state.hwnd));
                    }
                    // SAFETY: The native focus query observes only this current test thread.
                    assert_eq!(unsafe { GetFocus() }, engine.state.hwnd);
                    engine.check()?;
                }
                Ok(())
            });
            fields.close()?;
            // SAFETY: All fixture windows, text documents and callbacks have completed checked retirement.
            unsafe { apartment.release_after_teardown() }?;
            result?;
            assert_eq!(unsupported.load(std::sync::atomic::Ordering::SeqCst), 0);
            Ok(())
        },
    )
}

#[test]
fn native_field_updates_preserve_edits_selection_and_undo() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::native_field_updates_preserve_edits_selection_and_undo"),
        || {
            let mut apartment = UiApartment::enter()?;
            let mut fields = Fields::default();
            let result = (|| -> TestResult {
                fields.open()?;
                values::qualify(fields.engines.first().ok_or("first field absent")?)
            })();
            fields.close()?;
            // SAFETY: The fixture's original documents, callbacks and native windows retired.
            unsafe { apartment.release_after_teardown() }?;
            result
        },
    )
}

/// Native text admission and horizontal scrolling must preserve the visible insertion point.
fn exercise_long_field(engine: &Engine) -> TestResult {
    use windows::Win32::{
        Foundation::POINT,
        UI::Controls::{
            EM_GETLIMITTEXT, EM_REPLACESEL, EM_SCROLLCARET, EM_SETSEL, RichEdit::EM_GETSCROLLPOS,
        },
    };
    assert_eq!(engine.message(EM_GETLIMITTEXT, WPARAM(0), LPARAM(0))?.0, 4096);
    engine.message(EM_SETSEL, WPARAM(0), LPARAM(-1))?;
    let text: Vec<u16> = "界".repeat(4096).encode_utf16().chain([0]).collect();
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(text.as_ptr() as isize))?;
    assert_eq!(engine.message(WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0))?.0, 4096);
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(w!("X").as_ptr() as isize))?;
    assert_eq!(engine.message(WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0))?.0, 4096);
    engine.message(EM_SCROLLCARET, WPARAM(0), LPARAM(0))?;
    let mut position = POINT::default();
    engine.message(EM_GETSCROLLPOS, WPARAM(0), LPARAM((&raw mut position) as isize))?;
    assert!(position.x > 0);
    assert!(!engine.state.scroll_visible.get());
    let observed = caret()?;
    assert_eq!(observed.hwndCaret, engine.state.hwnd);
    assert!(observed.rcCaret.left >= 0 && observed.rcCaret.right <= 250);
    engine.check()?;
    Ok(())
}
