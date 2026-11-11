// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route registered native input through fixed callback storage on the creating thread.
use super::super::ReportSurfaceError;
use super::super::state::Intent;
use crate::windows_subclass::{callback, destroy_callback};
use std::cell::Cell;
mod queue;
use queue::Queue;
use sugarsubstitute_contracts::incident::{
    ExportSelection, ReportAction, ReportBinding, ReportRestartAvailability,
};
use tao::event_loop::EventLoopProxy;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{COLOR_WINDOW, FillRect, GetSysColorBrush, HDC},
    UI::{
        Controls::BST_CHECKED,
        Input::KeyboardAndMouse::{
            GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_INSERT, VK_SHIFT, VK_TAB,
        },
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            BM_GETCHECK, GetClientRect, GetNextDlgTabItem, SendMessageW, WM_COMMAND,
            WM_CONTEXTMENU, WM_COPY, WM_ERASEBKGND, WM_KEYDOWN, WM_NCDESTROY,
        },
    },
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Event {
    Wake,
}
#[derive(Clone, Copy)]
pub(super) struct Input {
    pub(super) binding: ReportBinding,
    pub(super) intent: Intent,
}
pub(super) struct Callback {
    pub(super) binding: Cell<ReportBinding>,
    pub(super) parent: HWND,
    pub(super) children: Cell<[HWND; 8]>,
    pub(super) enabled: Cell<bool>,
    pub(super) restart: Cell<ReportRestartAvailability>,
    proxy: EventLoopProxy<Event>,
    queue: Queue,
    failure: Cell<Option<ReportSurfaceError>>,
}
impl Callback {
    pub(super) fn new(parent: HWND, binding: ReportBinding, proxy: EventLoopProxy<Event>) -> Self {
        Self {
            parent,
            binding: Cell::new(binding),
            children: Cell::new([HWND::default(); 8]),
            enabled: Cell::new(false),
            restart: Cell::new(ReportRestartAvailability::Unavailable),
            proxy,
            queue: Queue::new(),
            failure: Cell::new(None),
        }
    }
    fn push(&self, intent: Intent) {
        if !self.enabled.get() || !intent.allowed(self.restart.get()) {
            return;
        }
        match self.queue.push(Input { binding: self.binding.get(), intent }) {
            Ok(true) => {
                if let Err(cause) = self.proxy.send_event(Event::Wake) {
                    self.fail(ReportSurfaceError::new("report_input_wake", cause));
                }
            }
            Ok(false) => {}
            Err(cause) => self.fail(cause),
        }
    }
    pub(super) fn take(&self) -> [Option<Input>; 32] {
        match self.queue.drain() {
            Ok(queue) => queue,
            Err(cause) => {
                self.fail(cause);
                [None; 32]
            }
        }
    }
    fn fail(&self, cause: ReportSurfaceError) {
        self.enabled.set(false);
        let first = self.failure.take();
        self.failure.set(Some(first.unwrap_or(cause)));
    }
    pub(super) fn take_failure(&self) -> Option<ReportSurfaceError> {
        self.failure.take()
    }
}
pub(super) unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: Exactly one installed Rc reference is reclaimed by the existing owner helper.
        return unsafe {
            destroy_callback::<Callback>(hwnd, Some(procedure), wparam, lparam, data)
        };
    }
    // SAFETY: Every installed subclass owns a live typed Rc callback through this invocation.
    let state = unsafe { callback::<Callback>(data) };
    if message == WM_ERASEBKGND && hwnd == state.parent {
        let mut bounds = RECT::default();
        // SAFETY: Paint only the retained parent with its current OS-owned system brush.
        let result = unsafe { GetClientRect(hwnd, &mut bounds) };
        match result {
            Ok(()) => {
                // SAFETY: WM_ERASEBKGND supplies a live borrowed HDC; no GDI ownership transfers.
                if unsafe {
                    FillRect(
                        HDC(wparam.0 as *mut std::ffi::c_void),
                        &bounds,
                        GetSysColorBrush(COLOR_WINDOW),
                    )
                } == 0
                {
                    state.fail(ReportSurfaceError::new(
                        "report_background_paint",
                        std::io::Error::other("system background fill returned zero"),
                    ));
                } else {
                    return LRESULT(1);
                }
            }
            Err(cause) => state.fail(ReportSurfaceError::new("report_background_bounds", cause)),
        }
    }
    if hwnd == state.children.get()[1] {
        // EDIT's stock copy/menu must not bypass current-view and host action fences.
        if message == WM_COPY {
            state.push(Intent::Action(ReportAction::CopyText));
            return LRESULT(0);
        }
        if message == WM_CONTEXTMENU {
            return LRESULT(0);
        }
        // SAFETY: Scalar keyboard state belongs to this current input thread.
        let control_down = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        if message == WM_KEYDOWN
            && control_down
            && (wparam.0 == 0x43 || wparam.0 == VK_INSERT.0 as usize)
        {
            state.push(Intent::Action(ReportAction::CopyText));
            return LRESULT(0);
        }
    }
    if message == WM_COMMAND && hwnd == state.parent && wparam.0 >> 16 == 0 {
        let children = state.children.get();
        let control = HWND(lparam.0 as *mut std::ffi::c_void);
        if let Some(index) =
            children.iter().position(|child| *child == control && !child.is_invalid())
        {
            match index {
                4 => state.push(Intent::Action(ReportAction::CopyText)),
                5 => {
                    // SAFETY: The exact retained checkbox is read synchronously on its owning thread.
                    let checked = unsafe { SendMessageW(children[2], BM_GETCHECK, None, None) };
                    let selection = if checked.0 == BST_CHECKED.0 as isize {
                        ExportSelection::TextAndNativeDump
                    } else {
                        ExportSelection::TextOnly
                    };
                    state.push(Intent::Export(selection));
                }
                6 => state.push(Intent::Action(ReportAction::Dismiss)),
                7 => state.push(Intent::Action(ReportAction::DismissAndRequestRestart)),
                _ => {}
            }
        }
    }
    if message == WM_KEYDOWN {
        if wparam.0 == VK_ESCAPE.0 as usize {
            state.push(Intent::Action(ReportAction::Dismiss));
            return LRESULT(0);
        }
        if wparam.0 == VK_TAB.0 as usize && hwnd != state.parent {
            // SAFETY: Native tab traversal only borrows this parent's retained child identities.
            match unsafe {
                GetNextDlgTabItem(state.parent, Some(hwnd), GetKeyState(VK_SHIFT.0 as i32) < 0)
            } {
                Ok(next) => {
                    // SAFETY: The chosen current native child is in this retained parent on the UI thread.
                    if let Err(cause) = unsafe { SetFocus(Some(next)) } {
                        state.fail(ReportSurfaceError::new("report_input_focus", cause));
                    }
                }
                Err(cause) => state.fail(ReportSurfaceError::new("report_input_traversal", cause)),
            }
            return LRESULT(0);
        }
    }
    // SAFETY: Preserve OS/Tao procedures for every unrelated native message.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
