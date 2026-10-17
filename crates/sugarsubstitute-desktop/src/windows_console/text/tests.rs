// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check real hidden RichEdit Unicode/style character space without focus or clipboard effects.
use super::*;
use crate::console::index::tests::{TestResult, index, source};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, ES_MULTILINE, ES_READONLY, WINDOW_EX_STYLE,
            WINDOW_STYLE,
        },
    },
    core::w,
};
struct Hidden {
    hwnd: Option<HWND>,
    module: Option<HMODULE>,
    document: Option<DocumentIndex>,
}
impl Hidden {
    fn new(document: DocumentIndex) -> Result<Self, DesktopError> {
        let mut owner = Self { hwnd: None, module: None, document: Some(document) };
        // SAFETY: System-only library and hidden test-thread HWND; every returned owner is retained.
        owner.module = Some(
            unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .map_err(|e| DesktopError::new("console_fixture_library", e))?,
        );
        // SAFETY: No visible/activation style, parent, callback or global input/clipboard effect.
        owner.hwnd = Some(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("RICHEDIT50W"),
                    w!(""),
                    WINDOW_STYLE((ES_MULTILINE | ES_READONLY) as u32),
                    0,
                    0,
                    300,
                    120,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .map_err(|e| DesktopError::new("console_fixture_window", e))?,
        );
        Ok(owner)
    }
    fn close(&mut self) -> Result<(), DesktopError> {
        if let Some(hwnd) = self.hwnd {
            // SAFETY: Exact hidden test-thread owner, retained on failure.
            unsafe { DestroyWindow(hwnd) }
                .map_err(|e| DesktopError::new("console_fixture_destroy", e))?;
            self.hwnd = None;
        }
        if let Some(module) = self.module {
            // SAFETY: Hidden RichEdit HWND has actually retired before its class module.
            unsafe { FreeLibrary(module) }
                .map_err(|e| DesktopError::new("console_fixture_free", e))?;
            self.module = None;
        }
        self.document = None;
        Ok(())
    }
}
impl Drop for Hidden {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            std::mem::forget(self.document.take());
            tracing::error!(operation="console_fixture_cleanup",cause=%cause,"Hidden console fixture remains retained");
        }
    }
}
#[test]
fn actual_readonly_native_document_preserves_unicode_nul_styles_and_selected_character_space()
-> TestResult {
    let (document, _frame, _released) =
        index(&source(1, &[(1, "a😀日本\0"), (2, "e\u{301} 끝")])?)?;
    let mut owner = Hidden::new(document)?;
    let hwnd = owner.hwnd.ok_or("hidden native peer absent")?;
    let mut port = NativeText { hwnd, dark: true };
    port.write(owner.document.as_ref().ok_or("original document absent")?)?;
    port.selection(1, 3)?;
    let selection = port.control().selection();
    assert_eq!(selection.cpMin, 1);
    assert_eq!(selection.cpMax, 3);
    let format = port.control().selected_format();
    assert!(format.Base.dwEffects.contains(CFE_BOLD));
    // SAFETY: Passive style observation of this exact hidden test-thread control.
    let style = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
        )
    };
    assert_ne!(style as u32 & ES_READONLY as u32, 0);
    owner.close()?;
    Ok(())
}
