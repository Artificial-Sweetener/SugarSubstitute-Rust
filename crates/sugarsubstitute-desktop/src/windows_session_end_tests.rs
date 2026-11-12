// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify session-end answers against the real Tao windows this owner depends on.
//!
//! Windows here are never shown: the application window is created hidden and attached
//! explicitly, so no test draws on the interactive desktop.
use super::*;
use std::{error::Error, rc::Rc};
use tao::{
    event_loop::EventLoopBuilder,
    platform::windows::{EventLoopBuilderExtWindows, WindowExtWindows},
    window::WindowBuilder,
};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

type TestResult = Result<(), Box<dyn Error>>;

/// Tao's version is pinned because its session-end internals are relied on; a change of
/// version must re-qualify the class name and the exit-on-WM_ENDSESSION behaviour.
#[test]
fn the_tao_version_this_owner_qualified_is_the_one_in_the_lockfile() {
    let lock = include_str!("../../../Cargo.lock").replace("\r\n", "\n");
    assert!(
        lock.contains("name = \"tao\"\nversion = \"0.37.0\""),
        "Tao changed: re-qualify its thread-target class and WM_ENDSESSION exit before updating"
    );
}

fn query(hwnd: HWND, flags: isize) -> LRESULT {
    // SAFETY: The window is live on this thread; the message carries no pointers.
    unsafe { SendMessageW(hwnd, WM_QUERYENDSESSION, None, Some(LPARAM(flags))) }
}

fn end(hwnd: HWND, ending: bool) -> LRESULT {
    // SAFETY: As above.
    unsafe { SendMessageW(hwnd, WM_ENDSESSION, Some(WPARAM(usize::from(ending))), Some(LPARAM(0))) }
}

/// On one real Tao loop: the thread target exists under its qualified class; it and the app
/// window answer the same way, FALSE while the close save is outstanding and TRUE once it
/// settled; only the app window shows the reason; WM_ENDSESSION never reaches Tao's exit; a
/// critical end is never held; a cancelled end removes the reason and is reported.
#[test]
fn every_watched_window_answers_a_session_end_from_the_close_save() -> TestResult {
    let event_loop = EventLoopBuilder::<()>::new().with_any_thread(true).build();
    let window = WindowBuilder::new().with_visible(false).build(&event_loop)?;
    let classes: Vec<_> = thread_windows().into_iter().filter_map(class_name).collect();
    assert!(
        classes.iter().any(|class| class == TAO_THREAD_TARGET_CLASS),
        "Tao's thread-target class changed: {classes:?}"
    );
    let wakes = Rc::new(Cell::new(0));
    let counted = wakes.clone();
    let mut watch =
        SessionEndWatch::install("Saving your workspace", move || counted.set(counted.get() + 1))?;
    let main = HWND(window.hwnd() as _);
    watch.attach(main)?;
    let target = watch.target.hwnd;

    for hwnd in [target, main] {
        assert_eq!(query(hwnd, 0), LRESULT(0), "a held session end answers FALSE");
    }
    assert_eq!(watch.take_signal(), Some(SessionEndSignal::Ending { critical: false }));
    assert_eq!(watch.state.blocking.borrow().as_slice(), &[main], "only the app window shows why");
    watch.set_outstanding(false);
    assert!(watch.state.blocking.borrow().is_empty(), "a settled close removes the reason");
    for hwnd in [target, main] {
        assert_eq!(query(hwnd, 0), LRESULT(1), "a settled close lets the session end");
    }
    // Reaching Tao here would end this test process with exit code 0.
    assert_eq!(end(target, true), LRESULT(0));
    assert_eq!(end(main, true), LRESULT(0));
    assert!(wakes.get() >= 4, "every query woke the event loop");

    watch.set_outstanding(true);
    assert_eq!(query(main, ENDSESSION_CRITICAL), LRESULT(1), "a critical end is never held");
    assert_eq!(watch.take_signal(), Some(SessionEndSignal::Ending { critical: true }));
    assert_eq!(query(main, 0), LRESULT(0));
    assert_eq!(end(main, false), LRESULT(0));
    assert_eq!(watch.take_signal(), Some(SessionEndSignal::Cancelled));
    assert!(watch.state.blocking.borrow().is_empty(), "a cancelled end removes the reason");
    watch.retire()?;
    Ok(())
}
