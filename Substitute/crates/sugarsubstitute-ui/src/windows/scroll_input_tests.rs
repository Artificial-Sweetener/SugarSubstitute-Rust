// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise shared scrollbar gestures with an explicit document and clock boundary.

use super::*;
use std::{cell::RefCell, collections::BTreeMap};
use windows::Win32::Foundation::{HWND, RECT};

#[derive(Default)]
struct Host {
    now: Cell<u64>,
    timers: RefCell<BTreeMap<usize, u32>>,
    failed_retirement: Cell<Option<usize>>,
    retired: RefCell<Vec<usize>>,
    steps: RefCell<Vec<ScrollStep>>,
    position: Cell<i32>,
    repaint: Cell<usize>,
}

#[test]
fn viewport_keys_use_the_original_document_range_and_page_commands()
-> Result<(), Box<dyn std::error::Error>> {
    use super::super::scroll_port::ScrollKey;
    let host = Host::default();
    let input = ScrollInput::new();
    assert!(input.key(&host, ScrollKey::End)?);
    assert_eq!(host.position.get(), 1000);
    assert!(input.key(&host, ScrollKey::PageBefore)?);
    assert!(input.key(&host, ScrollKey::PageAfter)?);
    assert_eq!(*host.steps.borrow(), [ScrollStep::PageBefore, ScrollStep::PageAfter]);
    assert!(input.key(&host, ScrollKey::Start)?);
    assert_eq!(host.position.get(), 0);
    assert_eq!(ScrollKey::from_virtual_key(0x21), Some(ScrollKey::PageBefore));
    assert_eq!(ScrollKey::from_virtual_key(0x22), Some(ScrollKey::PageAfter));
    assert_eq!(ScrollKey::from_virtual_key(0x24), Some(ScrollKey::Start));
    assert_eq!(ScrollKey::from_virtual_key(0x23), Some(ScrollKey::End));
    assert_eq!(ScrollKey::from_virtual_key(0x09), None);
    Ok(())
}
impl ScrollHost for Host {
    fn window(&self) -> HWND {
        HWND::default()
    }
    fn now_millis(&self) -> u64 {
        self.now.get()
    }
    fn observe(&self, hover: u8, pressed: Part) -> Result<Option<Model>, NativeUiError> {
        Ok(Some(Model {
            appearance: super::scrollbar::Appearance::Overlay,
            bounds: RECT { left: 0, top: 0, right: 100, bottom: 200 },
            scale: 1.0,
            minimum: 0,
            maximum: 1000,
            page: 200,
            position: self.position.get(),
            dark: true,
            enabled: true,
            hover,
            pressed,
        }))
    }
    fn scroll(&self, step: ScrollStep) -> Result<(), NativeUiError> {
        self.steps.borrow_mut().push(step);
        Ok(())
    }
    fn scroll_lines(&self, amount: i64) -> Result<(), NativeUiError> {
        self.position.set((i64::from(self.position.get()) + amount).clamp(0, 1000) as i32);
        Ok(())
    }
    fn set_position(&self, position: i32) -> Result<(), NativeUiError> {
        self.position.set(position.clamp(0, 1000));
        Ok(())
    }
    fn owns_timer(&self, id: usize) -> bool {
        self.timers.borrow().contains_key(&id)
    }
    fn set_timer(&self, id: usize, milliseconds: u32) -> Result<(), NativeUiError> {
        self.timers.borrow_mut().insert(id, milliseconds);
        Ok(())
    }
    fn kill_timer(&self, id: usize) -> Result<(), NativeUiError> {
        self.retired.borrow_mut().push(id);
        if self.failed_retirement.get() == Some(id) {
            self.failed_retirement.set(None);
            return Err(NativeUiError::message("retire_timer_fixture", "Retirement rejected"));
        }
        self.timers.borrow_mut().remove(&id);
        Ok(())
    }
    fn invalidate(&self) {
        self.repaint.set(self.repaint.get() + 1);
    }
}

#[test]
fn native_timer_delivery_uses_the_original_animation_and_ignores_retired_messages()
-> Result<(), Box<dyn std::error::Error>> {
    let input = ScrollInput::new();
    let host = Host::default();
    input.hover(&host, true)?;
    assert_eq!(host.timers.borrow().get(&ANIMATION), Some(&16));
    host.now.set(50);
    assert_eq!(input.model(&host)?.ok_or("Model absent")?.hover, 127);
    input.hover(&host, false)?;
    assert_eq!(input.model(&host)?.ok_or("Model absent")?.hover, 127);
    host.now.set(150);
    assert!(input.message(&host, WM_TIMER, WPARAM(ANIMATION), LPARAM(0))?);
    assert_eq!(input.model(&host)?.ok_or("Model absent")?.hover, 0);
    assert!(!host.owns_timer(ANIMATION));
    let repaint = host.repaint.get();
    assert!(!input.message(&host, WM_TIMER, WPARAM(ANIMATION), LPARAM(0))?);
    assert_eq!(host.repaint.get(), repaint);
    Ok(())
}

#[test]
fn failed_timer_cleanup_retains_its_obligation_and_attempts_the_other_timer()
-> Result<(), Box<dyn std::error::Error>> {
    let input = ScrollInput::new();
    let host = Host::default();
    for id in [ANIMATION, REPEAT, 42] {
        host.set_timer(id, 16)?;
    }
    host.failed_retirement.set(Some(ANIMATION));
    assert!(input.cancel(&host, true).is_err());
    assert_eq!(*host.retired.borrow(), [ANIMATION, REPEAT]);
    assert!(host.owns_timer(ANIMATION));
    assert!(!host.owns_timer(REPEAT));
    assert!(host.owns_timer(42));
    input.cancel(&host, true)?;
    assert!(!host.owns_timer(ANIMATION));
    assert!(host.owns_timer(42));
    Ok(())
}

#[test]
fn repeat_scrolls_only_while_the_original_pressed_region_still_matches()
-> Result<(), Box<dyn std::error::Error>> {
    let input = ScrollInput::new();
    let host = Host::default();
    input.pressed.set(Part::PageAfter);
    input.point.set(POINT { x: 95, y: 150 });
    host.set_timer(REPEAT, 500)?;
    assert!(input.message(&host, WM_TIMER, WPARAM(REPEAT), LPARAM(0))?);
    assert_eq!(*host.steps.borrow(), [ScrollStep::PageAfter]);
    assert_eq!(host.timers.borrow().get(&REPEAT), Some(&50));
    input.point.set(POINT { x: 10, y: 150 });
    assert!(input.message(&host, WM_TIMER, WPARAM(REPEAT), LPARAM(0))?);
    assert_eq!(host.steps.borrow().len(), 1);
    input.position(&host, 317)?;
    assert_eq!(host.position.get(), 317);
    input.cancel(&host, true)?;
    assert!(!input.message(&host, WM_TIMER, WPARAM(REPEAT), LPARAM(0))?);
    Ok(())
}
#[test]
fn hover_endpoints_reverse_from_current_value_without_wall_clock_waits() {
    assert_eq!(progress(0, 255, 0), 0);
    assert_eq!(progress(0, 255, 50), 127);
    assert_eq!(progress(0, 255, 100), 255);
    assert_eq!(progress(0, 255, u64::MAX), 255);
    assert_eq!(progress(127, 0, 0), 127);
    assert_eq!(progress(127, 0, 100), 0);
    assert_eq!(progress(255, 0, 50), 128);
    const {
        assert!(usize::BITS >= 64);
        assert!(ANIMATION > u32::MAX as usize);
        assert!(REPEAT > u32::MAX as usize);
    }
    assert_ne!(ANIMATION, REPEAT);
}
