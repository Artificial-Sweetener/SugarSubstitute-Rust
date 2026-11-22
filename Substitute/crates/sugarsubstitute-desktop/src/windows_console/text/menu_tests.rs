// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open the console's real popup menu over a real console on the private test desktop.
use super::{Hidden, TestResult, UiApartment, index, source};
use crate::windows_controls::console::{
    menu::{Document, Request, Row, choose, run},
    messages::Messages,
};
use std::cell::{Cell, RefCell};
use sugarsubstitute_contracts::localization::edit::EN;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        UI::WindowsAndMessaging::{
            EndMenu, FindWindowW, GetMenuItemCount, GetMenuState, GetMenuStringW, GetWindowRect,
            HMENU, KillTimer, MF_BYPOSITION, MF_GRAYED, MF_SEPARATOR, MN_GETHMENU, SendMessageW,
            SetTimer, WM_CONTEXTMENU, WM_RBUTTONDOWN, WM_RBUTTONUP,
        },
    },
    core::w,
};

/// One row as the open popup shows it: label, disabled, separator.
type Shown = (String, bool, bool);

thread_local! {
    static SHOWN: RefCell<Vec<Shown>> = const { RefCell::new(Vec::new()) };
    static MENU_REQUESTS: Cell<usize> = const { Cell::new(0) };
}

/// Stand where the console's own procedure stands, outside the text engine, and count menus.
unsafe extern "system" fn requests(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_CONTEXTMENU {
        MENU_REQUESTS.with(|count| count.set(count.get() + 1));
        return LRESULT(0);
    }
    // SAFETY: Every other message keeps the fixture's native chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Read the open popup's rows, then close it without a choice.
unsafe extern "system" fn inspect(_: HWND, _: u32, timer: usize, _: u32) {
    // SAFETY: The popup's modal loop on this thread dispatches this timer while the menu is open.
    unsafe {
        let _stopped = KillTimer(None, timer);
        if let Ok(popup) = FindWindowW(w!("#32768"), None) {
            let menu = HMENU(SendMessageW(popup, MN_GETHMENU, None, None).0 as *mut _);
            for position in 0..GetMenuItemCount(Some(menu)).max(0) as u32 {
                let mut text = [0u16; 64];
                let length = GetMenuStringW(menu, position, Some(&mut text), MF_BYPOSITION);
                let state = GetMenuState(menu, position, MF_BYPOSITION);
                let label = String::from_utf16_lossy(&text[..length.max(0) as usize]);
                let shown = (label, state & MF_GRAYED.0 != 0, state & MF_SEPARATOR.0 != 0);
                SHOWN.with(|rows| rows.borrow_mut().push(shown));
            }
        }
        let _closed = EndMenu();
    }
}

/// The page's rows in its order; a read-only console enables only Copy and Select all.
#[test]
fn console_menu_offers_the_page_rows_and_runs_its_choice() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::console_menu_offers_the_page_rows_and_runs_its_choice"),
        qualify_menu,
    )
}

fn qualify_menu() -> TestResult {
    use super::super::publish;
    use crate::console::update::Documents;
    use windows::Win32::UI::WindowsAndMessaging::{
        GA_PARENT, GetAncestor, SW_SHOWNOACTIVATE, ShowWindow,
    };
    let mut apartment = UiApartment::enter()?;
    let (document, _frame, _release) = index(&source(1, &[(1, "menu target text")])?)?;
    let mut owner = Hidden::new(document, &apartment)?;
    let hwnd = owner.hwnd.ok_or("native child absent")?;
    let mut documents = Documents::new(owner.document.take().ok_or("document absent")?);
    publish(hwnd, &mut documents, true)?;
    // SAFETY: Both exact fixture windows belong exclusively to the owned private desktop.
    unsafe {
        let _previous = ShowWindow(GetAncestor(hwnd, GA_PARENT), SW_SHOWNOACTIVATE);
        let _previous = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    // A right-button release passes through the text engine and asks the console for its menu.
    // SAFETY: The recorder is installed and removed on this thread around one synchronous click.
    unsafe {
        assert!(SetWindowSubclass(hwnd, Some(requests), 77, 0).as_bool());
        SendMessageW(hwnd, WM_RBUTTONDOWN, Some(WPARAM(2)), Some(LPARAM(10 << 16 | 20)));
        SendMessageW(hwnd, WM_RBUTTONUP, Some(WPARAM(0)), Some(LPARAM(10 << 16 | 20)));
        assert!(RemoveWindowSubclass(hwnd, Some(requests), 77).as_bool());
    }
    assert_eq!(MENU_REQUESTS.get(), 1, "a right-click did not reach the console as a menu request");
    let control = Messages(hwnd);
    control.select(0, 4)?;
    let request = Request::new(hwnd, LPARAM(-1), &EN, true);
    assert_eq!(request.document, Document { selected: true, text: true });
    let mut bounds = RECT::default();
    // SAFETY: Read the exact fixture child's screen rectangle.
    unsafe { GetWindowRect(hwnd, &mut bounds) }?;
    let point = request.point;
    assert!(
        (bounds.left..bounds.right).contains(&point.x)
            && (bounds.top..bounds.bottom).contains(&point.y),
        "keyboard menu opens outside the console: {point:?} in {bounds:?}"
    );

    // SAFETY: A thread timer reaches `inspect` through the popup's own modal loop.
    unsafe { SetTimer(None, 0, 10, Some(inspect)) };
    assert_eq!(choose(hwnd, &request)?, None);
    let shown = SHOWN.take();
    let rows: Vec<_> = shown
        .iter()
        .map(|(label, disabled, separator)| (label.as_str(), *separator || *disabled, *separator))
        .collect();
    assert_eq!(
        rows,
        [
            ("Undo", true, false),
            ("Redo", true, false),
            ("", true, true),
            ("Cut", true, false),
            ("Copy", false, false),
            ("Paste", true, false),
            ("Select all", false, false),
        ]
    );

    run(hwnd, Row::SelectAll)?;
    // As with Ctrl+A, the selection also takes the document's closing paragraph mark.
    let all = control.selection();
    assert!(all.cpMin == 0 && all.cpMax as isize >= control.length(), "selected {all:?}");
    // Output keeps arriving during start-up: a selection on the closing mark must survive it.
    let (more, _more_frame, _more_release) =
        index(&source(2, &[(1, "menu target text"), (2, "more output")])?)?;
    documents.stage(more)?;
    publish(hwnd, &mut documents, true)?;
    // Ctrl+A reaches the same native end.
    // SAFETY: Select through the live fixture console, as its Ctrl+A does.
    unsafe {
        SendMessageW(
            hwnd,
            windows::Win32::UI::Controls::EM_SETSEL,
            Some(WPARAM(0)),
            Some(LPARAM(-1)),
        );
    }
    let (most, _most_frame, _most_release) =
        index(&source(3, &[(1, "menu target text"), (2, "more output"), (3, "most")])?)?;
    documents.stage(most)?;
    publish(hwnd, &mut documents, true)?;
    control.select(2, 2)?;
    assert_eq!(Document::read(hwnd), Document { selected: false, text: true });
    assert!(!Row::Copy.enabled(Document::read(hwnd)));
    assert!(Row::SelectAll.enabled(Document::read(hwnd)));

    owner.close()?;
    drop(owner);
    // SAFETY: The exact native child, parent, and services have retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}
