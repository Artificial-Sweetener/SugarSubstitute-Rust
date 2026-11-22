// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Offer the page's text edit rows on the read-only native console.
//!
//! The rows and their order match every text menu of the page. The console is read-only, so
//! only Copy (with a selection) and Select all (with text) can run; the others show disabled.
use super::messages::Messages;
use crate::DesktopError;
use sugarsubstitute_contracts::localization::edit::EditMessages;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
        UI::{
            Controls::EM_POSFROMCHAR,
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, DestroyMenu, GetClientRect, HMENU, MF_GRAYED,
                MF_SEPARATOR, MF_STRING, SendMessageW, TPM_NONOTIFY, TPM_RETURNCMD,
                TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_COPY,
            },
        },
    },
    core::{HSTRING, PCSTR, w},
};

/// One edit row of the shared text menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    Undo = 1,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl Row {
    /// The page's row order; `None` is its one separator.
    pub(super) const ORDER: [Option<Self>; 7] = [
        Some(Self::Undo),
        Some(Self::Redo),
        None,
        Some(Self::Cut),
        Some(Self::Copy),
        Some(Self::Paste),
        Some(Self::SelectAll),
    ];

    fn label(self, names: &EditMessages) -> &'static str {
        match self {
            Self::Undo => names.undo,
            Self::Redo => names.redo,
            Self::Cut => names.cut,
            Self::Copy => names.copy,
            Self::Paste => names.paste,
            Self::SelectAll => names.select_all,
        }
    }

    fn chosen(command: i32) -> Option<Self> {
        Self::ORDER.into_iter().flatten().find(|row| *row as i32 == command)
    }

    /// Whether a read-only document can run this row.
    pub(super) fn enabled(self, document: Document) -> bool {
        match self {
            Self::Copy => document.selected,
            Self::SelectAll => document.text,
            Self::Undo | Self::Redo | Self::Cut | Self::Paste => false,
        }
    }
}

/// What the console holds when its menu opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Document {
    pub(super) selected: bool,
    pub(super) text: bool,
}

impl Document {
    pub(super) fn read(hwnd: HWND) -> Self {
        let control = Messages(hwnd);
        let range = control.selection();
        Self { selected: range.cpMin != range.cpMax, text: control.length() > 0 }
    }
}

/// Everything the menu needs, read before its modal loop can retire the console.
pub(super) struct Request {
    pub(super) point: POINT,
    pub(super) document: Document,
    pub(super) names: &'static EditMessages,
    pub(super) dark: bool,
}

impl Request {
    /// Place a pointer menu at the pointer and a keyboard menu (`lparam` -1) at the selection.
    pub(super) fn new(
        hwnd: HWND,
        lparam: LPARAM,
        names: &'static EditMessages,
        dark: bool,
    ) -> Self {
        let packed = lparam.0 as u32;
        let point = if lparam.0 == -1 {
            keyboard_anchor(hwnd)
        } else {
            POINT { x: i32::from(packed as u16 as i16), y: i32::from((packed >> 16) as u16 as i16) }
        };
        Self { point, document: Document::read(hwnd), names, dark }
    }
}

/// The selection's start when it is in view, else the console's top-left corner, on screen.
fn keyboard_anchor(hwnd: HWND) -> POINT {
    let start = Messages(hwnd).selection().cpMin;
    let mut at = POINT::default();
    let mut client = RECT::default();
    // SAFETY: The live console writes this stack point and rectangle synchronously.
    unsafe {
        SendMessageW(
            hwnd,
            EM_POSFROMCHAR,
            Some(WPARAM((&raw mut at) as usize)),
            Some(LPARAM(start as isize)),
        );
        if GetClientRect(hwnd, &mut client).is_err()
            || at.x < client.left
            || at.x >= client.right
            || at.y < client.top
            || at.y >= client.bottom
        {
            at = POINT::default();
        }
        let _mapped = ClientToScreen(hwnd, &mut at);
    }
    at
}

/// Destroy the popup menu however its use ends.
struct Popup(HMENU);
impl Drop for Popup {
    fn drop(&mut self) {
        // SAFETY: This owner created the menu, and no window holds it after the modal call.
        if let Err(cause) = unsafe { DestroyMenu(self.0) } {
            tracing::error!(operation = "console_menu_destroy", cause = %cause);
        }
    }
}

/// Show the menu and return the chosen row.
///
/// The popup runs its own modal loop, which dispatches other messages: the console can retire
/// before this returns, so the caller checks the console again before running the row.
pub(super) fn choose(owner: HWND, request: &Request) -> Result<Option<Row>, DesktopError> {
    // SAFETY: Creating an unowned popup menu has no preconditions.
    let popup = Popup(unsafe { CreatePopupMenu() }.map_err(|e| failure("console_menu_create", e))?);
    for entry in Row::ORDER {
        let appended = match entry {
            // SAFETY: This owner's live menu takes a separator, which carries no text.
            None => unsafe { AppendMenuW(popup.0, MF_SEPARATOR, 0, None) },
            Some(row) => {
                let flags =
                    if row.enabled(request.document) { MF_STRING } else { MF_STRING | MF_GRAYED };
                let label = HSTRING::from(row.label(request.names));
                // SAFETY: The label outlives this synchronous append; the menu copies the text.
                unsafe { AppendMenuW(popup.0, flags, row as usize, &label) }
            }
        };
        appended.map_err(|e| failure("console_menu_append", e))?;
    }
    follow_theme(request.dark);
    // SAFETY: The owner is the live console on this UI thread; the menu returns its command.
    let command = unsafe {
        TrackPopupMenuEx(
            popup.0,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0,
            request.point.x,
            request.point.y,
            owner,
            None,
        )
    };
    Ok(Row::chosen(command.0))
}

/// Run a chosen row on the console, which must still be the one the menu opened over. A row the
/// console cannot run now does nothing, whatever the popup returned.
pub(super) fn run(hwnd: HWND, row: Row) -> Result<(), DesktopError> {
    if !row.enabled(Document::read(hwnd)) {
        return Ok(());
    }
    match row {
        // SAFETY: The live console copies its own selection, as for Ctrl+C.
        Row::Copy => unsafe {
            SendMessageW(hwnd, WM_COPY, None, None);
        },
        Row::SelectAll => Messages(hwnd).select(0, -1)?,
        Row::Undo | Row::Redo | Row::Cut | Row::Paste => {}
    }
    Ok(())
}

/// Draw popup menus in the console's theme.
///
/// Windows themes popup menus only through two unnamed uxtheme exports: ordinal 135 sets the
/// process's preferred menu theme and 136 drops cached menu themes. Without them the menu keeps
/// the system's look.
fn follow_theme(dark: bool) {
    const FORCE_DARK: i32 = 2;
    const FORCE_LIGHT: i32 = 3;
    // SAFETY: uxtheme is a system module the process has loaded; both ordinals have these
    // signatures on every Windows version that exports them.
    unsafe {
        let Ok(module) = GetModuleHandleW(w!("uxtheme.dll")) else {
            return;
        };
        let (Some(prefer), Some(flush)) = (
            GetProcAddress(module, PCSTR(135 as *const u8)),
            GetProcAddress(module, PCSTR(136 as *const u8)),
        ) else {
            return;
        };
        let prefer: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(prefer);
        let flush: unsafe extern "system" fn() = std::mem::transmute(flush);
        prefer(if dark { FORCE_DARK } else { FORCE_LIGHT });
        flush();
    }
}

fn failure(operation: &'static str, cause: windows::core::Error) -> DesktopError {
    DesktopError::new(operation, cause)
}
