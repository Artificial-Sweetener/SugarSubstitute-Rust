// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise bounded selection custody without opening a shell dialog or showing a window.
use super::*;
use windows::{
    Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    },
    core::w,
};

/// Retain this thread's invisible peer through native fixture dispatch.
struct Window(HWND);
impl Window {
    /// Create a private peer without changing visibility, activation or pointer state.
    fn new() -> Result<Self, Box<dyn Error>> {
        // SAFETY: This thread owns an explicitly invisible STATIC peer, without activation styles.
        Ok(Self(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )?
        }))
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: The private peer remains on its creating thread until this final destruction.
        if let Err(cause) = unsafe { DestroyWindow(self.0) } {
            tracing::error!(operation="picker_fixture_retirement", cause=%cause, "Hidden peer destruction failed");
        }
    }
}
/// Replace the shell while retaining the production request/result owner.
fn fixture(window: HWND, select: Selection) -> Result<Picker, Box<dyn Error>> {
    let mut builder = EventLoopBuilder::<()>::with_user_event();
    let picker = Picker::with_selection(&mut builder, select);
    picker.bind(window, || Ok(()))?;
    Ok(picker)
}

#[test]
fn queued_cancel_never_opens_shell_and_result_has_one_consumer() -> Result<(), Box<dyn Error>> {
    let window = Window::new()?;
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let picker = fixture(
        window.0,
        Box::new(move |_, _, _| {
            calls.set(calls.get() + 1);
            Ok(None)
        }),
    )?;
    picker.request(SelectionKind::File)?;
    assert!(picker.busy());
    assert!(picker.request(SelectionKind::Folder).is_err());
    assert!(picker.retire().is_err());
    picker.cancel()?;
    assert!(picker.busy());
    picker.0.message_turn();
    assert_eq!(observed.get(), 0);
    assert_eq!(picker.take_result().transpose()?, Some(None));
    assert!(!picker.busy());
    assert!(picker.take_result().is_none());
    picker.retire()?;
    assert!(picker.request(SelectionKind::File).is_err());
    assert!(picker.bind(window.0, || Ok(())).is_err());
    Ok(())
}

#[test]
fn native_result_keeps_original_path_units_and_shell_error() -> Result<(), Box<dyn Error>> {
    let window = Window::new()?;
    let path = PathBuf::from(OsString::from_wide(&[0x61, 0xd800, 0x62]));
    let expected = path.clone();
    let picker = fixture(
        window.0,
        Box::new(move |_, kind, _| {
            if kind == SelectionKind::File {
                Ok(Some(path.clone()))
            } else {
                Err(std::io::Error::from_raw_os_error(5).into())
            }
        }),
    )?;
    picker.request(SelectionKind::File)?;
    assert!(picker.take_result().is_none());
    picker.0.message_turn();
    assert!(picker.busy());
    assert_eq!(picker.take_result().transpose()?, Some(Some(expected)));
    picker.request(SelectionKind::Folder)?;
    picker.0.message_turn();
    let cause =
        picker.take_result().ok_or("Missing shell outcome")?.err().ok_or("Missing shell cause")?;
    assert_eq!(
        cause.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    picker.retire()?;
    Ok(())
}

#[test]
fn cancellation_during_native_selection_cannot_retire_or_publish_early()
-> Result<(), Box<dyn Error>> {
    let window = Window::new()?;
    let owner = Rc::new(RefCell::new(None::<Picker>));
    let nested = owner.clone();
    let picker = fixture(
        window.0,
        Box::new(move |_, _, _| {
            let slot = nested.borrow();
            let picker = slot.as_ref().ok_or("Missing original picker")?;
            assert!(picker.busy());
            assert!(picker.take_result().is_none());
            assert!(picker.retire().is_err());
            assert!(picker.request(SelectionKind::File).is_err());
            picker.cancel()?;
            assert!(picker.busy());
            assert!(picker.take_result().is_none());
            Ok(None)
        }),
    )?;
    *owner.borrow_mut() = Some(picker.clone());
    picker.request(SelectionKind::File)?;
    picker.0.message_turn();
    assert_eq!(picker.take_result().transpose()?, Some(None));
    owner.borrow_mut().take();
    picker.retire()?;
    Ok(())
}

#[test]
fn wake_failure_does_not_replace_the_owned_shell_result() -> Result<(), Box<dyn Error>> {
    let window = Window::new()?;
    let mut builder = EventLoopBuilder::<()>::with_user_event();
    let picker = Picker::with_selection(
        &mut builder,
        Box::new(|_, _, _| Ok(Some(PathBuf::from("selected.png")))),
    );
    picker.bind(window.0, || Err(DesktopError::message("picker_wake_fixture", "wake rejected")))?;
    assert!(picker.bind(window.0, || Ok(())).is_err());
    picker.request(SelectionKind::File)?;
    picker.0.message_turn();
    assert!(picker.check().is_err());
    assert!(picker.check().is_ok());
    assert_eq!(picker.take_result().transpose()?, Some(Some(PathBuf::from("selected.png"))));
    picker.retire()?;
    Ok(())
}

#[test]
fn a_released_picker_serves_the_next_window_and_never_the_old_one() -> Result<(), Box<dyn Error>> {
    let first = Window::new()?;
    let second = Window::new()?;
    let windows = Rc::new(RefCell::new(Vec::new()));
    let seen = windows.clone();
    let picker = fixture(
        first.0,
        Box::new(move |window, _, _| {
            seen.borrow_mut().push(window);
            Ok(None)
        }),
    )?;
    picker.request(SelectionKind::File)?;
    assert!(picker.release().is_err(), "a pending request keeps its window");
    picker.0.message_turn();
    assert_eq!(picker.take_result().transpose()?, Some(None));
    picker.release()?;
    assert!(picker.request(SelectionKind::File).is_err(), "no window is bound");
    picker.0.message_turn();
    picker.bind(second.0, || Ok(()))?;
    picker.request(SelectionKind::File)?;
    picker.0.message_turn();
    assert_eq!(picker.take_result().transpose()?, Some(None));
    assert_eq!(*windows.borrow(), vec![first.0, second.0]);
    picker.retire()?;
    assert!(picker.bind(first.0, || Ok(())).is_err());
    Ok(())
}
