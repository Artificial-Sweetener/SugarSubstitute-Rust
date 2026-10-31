// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise drag mapping, registration ordering and a real hidden-window OLE registration.

use super::super::callbacks::tests::fixture_events;
use super::*;
use windows::{
    Win32::{
        Foundation::{DRAGDROP_E_ALREADYREGISTERED, E_FAIL},
        System::{
            Ole::{DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE},
            SystemServices::{MK_CONTROL, MK_LBUTTON},
        },
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE},
    },
    core::w,
};

#[test]
fn browser_drop_effects_stay_within_the_source_allowed_effects() {
    let allowed = DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_LINK.0);
    assert_eq!(effect_within(allowed, DROPEFFECT_COPY), DROPEFFECT_COPY);
    assert_eq!(effect_within(allowed, DROPEFFECT_MOVE), DROPEFFECT_NONE);
    assert_eq!(
        effect_within(allowed, DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_LINK.0)),
        DROPEFFECT_LINK
    );
    assert_eq!(
        effect_within(DROPEFFECT_COPY, DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_SCROLL.0)),
        DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_SCROLL.0)
    );
    assert_eq!(effect_within(DROPEFFECT_NONE, DROPEFFECT_COPY), DROPEFFECT_NONE);
}

#[test]
fn screen_drag_points_become_signed_browser_client_points() {
    assert_eq!(
        browser_point(POINTL { x: 1930, y: 260 }, POINT { x: 1920, y: 200 }),
        Some(POINT { x: 10, y: 60 })
    );
    assert_eq!(
        browser_point(POINTL { x: -5, y: 300 }, POINT { x: -1910, y: 400 }),
        Some(POINT { x: 1905, y: -100 })
    );
    assert_eq!(browser_point(POINTL { x: i32::MIN, y: 0 }, POINT { x: 1, y: 0 }), None);
}

/// Record native effects and the forwarded controller release in one ordered log.
struct Recorder {
    log: Rc<RefCell<Vec<&'static str>>>,
    register: Result<(), windows::core::HRESULT>,
    revoke: Vec<Result<(), windows::core::HRESULT>>,
}

impl Registrar for Recorder {
    fn initialize(&mut self) -> windows::core::Result<()> {
        self.log.borrow_mut().push("initialize");
        Ok(())
    }
    fn register(&mut self) -> windows::core::Result<()> {
        self.log.borrow_mut().push("register");
        self.register.map_err(windows::core::Error::from)
    }
    fn revoke(&mut self) -> windows::core::Result<()> {
        self.log.borrow_mut().push("revoke");
        let outcome = if self.revoke.is_empty() { Ok(()) } else { self.revoke.remove(0) };
        outcome.map_err(windows::core::Error::from)
    }
    fn uninitialize(&mut self) {
        self.log.borrow_mut().push("uninitialize");
    }
}

#[test]
fn teardown_revokes_before_releasing_the_controller_and_balancing_ole()
-> Result<(), Box<dyn std::error::Error>> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut lifecycle = Lifecycle::open(Recorder {
        log: log.clone(),
        register: Ok(()),
        revoke: vec![Err(E_FAIL), Ok(())],
    })?;
    assert!(lifecycle.retire(|| log.borrow_mut().push("release")).is_err());
    assert_eq!(*log.borrow(), ["initialize", "register", "revoke"]);
    lifecycle.retire(|| log.borrow_mut().push("release"))?;
    lifecycle.retire(|| log.borrow_mut().push("release"))?;
    assert_eq!(
        *log.borrow(),
        ["initialize", "register", "revoke", "revoke", "release", "uninitialize"]
    );
    assert_eq!(lifecycle.stage, Stage::Released);
    Ok(())
}

#[test]
fn rejected_registration_balances_ole_without_revoking() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let opened = Lifecycle::open(Recorder {
        log: log.clone(),
        register: Err(DRAGDROP_E_ALREADYREGISTERED),
        revoke: Vec::new(),
    });
    assert!(opened.is_err());
    assert_eq!(*log.borrow(), ["initialize", "register", "uninitialize"]);
}

#[test]
fn registrations_removed_by_window_destruction_settle_retirement() {
    for code in [DRAGDROP_E_NOTREGISTERED, DRAGDROP_E_INVALIDHWND] {
        assert!(settled(Err(code.into())).is_ok());
    }
    assert_eq!(settled(Err(E_FAIL.into())).map_err(|cause| cause.code()), Err(E_FAIL));
}

/// Keep an isolated hidden HWND alive without display, focus or Tao-loop dependencies.
struct HiddenWindow(HWND);

impl HiddenWindow {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // SAFETY: STATIC is an OS class; no styles make the owned window visible.
        Ok(Self(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Browser drop target fixture"),
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

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        // SAFETY: This test thread owns the hidden HWND.
        if let Err(cause) = unsafe { DestroyWindow(self.0) } {
            tracing::error!(operation = "destroy_drop_target_fixture", cause = %cause,
                "Native fixture destruction failed");
        }
    }
}

#[test]
fn real_registration_owns_the_window_until_revoked_and_declines_without_a_browser()
-> Result<(), Box<dyn std::error::Error>> {
    let window = HiddenWindow::new()?;
    let fixture = fixture_events(Rc::new(Cell::new(0)))?;
    let state = Rc::new(State {
        window: window.0,
        controller: RefCell::new(None),
        events: fixture.events.clone(),
        active: Cell::new(true),
        entered: Cell::new(false),
        callbacks: CallbackFence::default(),
    });
    let target: IDropTarget = Target(state.clone()).into();
    let mut lifecycle = Lifecycle::open(Native { window: window.0, target: target.clone() })?;
    let competing: IDropTarget = Target(state.clone()).into();
    // SAFETY: This test thread owns the hidden HWND and initialized OLE through the lifecycle.
    let duplicate = unsafe { RegisterDragDrop(window.0, &competing) };
    assert_eq!(duplicate.map_err(|cause| cause.code()), Err(DRAGDROP_E_ALREADYREGISTERED));

    let mut effect = DROPEFFECT_COPY;
    let keys = MODIFIERKEYS_FLAGS(MK_LBUTTON.0 | MK_CONTROL.0);
    // SAFETY: The registered target is UI-affine to this thread; the effect is live storage.
    unsafe {
        target.DragEnter(None::<&IDataObject>, keys, POINTL { x: 4, y: 5 }, &raw mut effect)?
    };
    assert_eq!(effect, DROPEFFECT_NONE);
    assert!(!state.entered.get());
    // SAFETY: A null effect pointer must be rejected rather than written.
    let rejected = unsafe { target.DragOver(keys, POINTL { x: 4, y: 5 }, std::ptr::null_mut()) };
    assert_eq!(rejected.map_err(|cause| cause.code()), Err(E_POINTER));

    let observed = Cell::new(None);
    lifecycle.retire(|| {
        // SAFETY: OLE remains initialized during release; this observes the emptied registration.
        let revoked = unsafe { RevokeDragDrop(window.0) };
        observed.set(Some(revoked.map_err(|cause| cause.code())));
    })?;
    assert_eq!(observed.get(), Some(Err(DRAGDROP_E_NOTREGISTERED)));
    Ok(())
}
