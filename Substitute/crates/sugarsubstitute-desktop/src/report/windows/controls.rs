// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own OS text, buttons, consent and every subclass borrow until checked teardown.
use super::super::{NativeReportView, ReportSurfaceError};
use super::{
    buffer::Text,
    callback::{Callback, Event, procedure},
    clipboard::Clipboard,
};
use crate::windows_subclass::{Registration, install, remove};
use std::{ffi::c_void, rc::Rc};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    incident::ReportRestartAvailability,
    localization::incident::ReportMessages,
};
use sugarsubstitute_ui::windows::font::{FontRole, Fonts};
use tao::{event_loop::EventLoopProxy, platform::windows::WindowExtWindows, window::Window};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Controls::{BST_UNCHECKED, EM_SETLIMITTEXT},
            HiDpi::GetDpiForWindow,
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{
                BM_SETCHECK, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CreateWindowExW, DestroyWindow,
                ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, HMENU, SWP_NOACTIVATE, SWP_NOZORDER,
                SendMessageW, SetWindowPos, SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE,
                WM_GETFONT, WM_SETFONT, WS_CHILD, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
            },
        },
    },
    core::{PCWSTR, w},
};
struct Peer {
    hwnd: HWND,
    registration: Registration<Callback>,
}
pub(super) struct Controls {
    pub(super) window: Rc<Window>,
    pub(super) callback: Rc<Callback>,
    parent: Registration<Callback>,
    peers: [Option<Peer>; 8],
    pub(super) clipboard: Clipboard,
    pub(super) lease: MemoryLease,
    fonts: Fonts,
    _claim: MemoryClaim,
}
impl Controls {
    pub(super) fn new(
        window: Rc<Window>,
        view: &NativeReportView,
        proxy: EventLoopProxy<Event>,
        lease: MemoryLease,
    ) -> Result<Self, ReportSurfaceError> {
        let claim = lease
            .claim((size_of::<Self>() + size_of::<Callback>() + 4096) as u64)
            .map_err(|cause| ReportSurfaceError::new("report_native_header_credit", cause))?;
        let hwnd = HWND(window.hwnd() as *mut c_void);
        let callback = Rc::new(Callback::new(hwnd, view.binding(), proxy));
        let parent = Registration::new(&callback);
        Ok(Self {
            window,
            callback,
            parent,
            peers: std::array::from_fn(|_| None),
            clipboard: Clipboard::new(hwnd),
            lease,
            fonts: Fonts::new(FontRole::Message),
            _claim: claim,
        })
    }
    pub(super) fn initialize(
        &mut self,
        view: &NativeReportView,
        labels: &ReportMessages,
    ) -> Result<(), ReportSurfaceError> {
        let parent = self.callback.parent;
        // SAFETY: Stable typed Rc data and retained parent live on the creating thread.
        unsafe { install(parent, Some(procedure), &self.parent) }?;
        let child_labels = [
            labels.explanation,
            view.text(),
            labels.include_dump,
            labels.dump_warning,
            labels.copy,
            labels.export,
            labels.close,
            labels.restart,
        ];
        for (index, label) in child_labels.into_iter().enumerate() {
            let text = Text::new(label, &self.lease)?;
            let (class, style) = match index {
                1 => (
                    w!("EDIT"),
                    WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32)
                        | WS_VSCROLL
                        | WS_TABSTOP,
                ),
                2 => (w!("BUTTON"), WINDOW_STYLE(BS_AUTOCHECKBOX as u32) | WS_TABSTOP),
                4..=7 => (w!("BUTTON"), WINDOW_STYLE(BS_PUSHBUTTON as u32) | WS_TABSTOP),
                _ => (w!("STATIC"), WINDOW_STYLE::default()),
            };
            // SAFETY: OS class/style, bounded terminated data and live retained parent.
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
                    PCWSTR(text.units.as_ptr()),
                    style | WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    1,
                    1,
                    Some(parent),
                    Some(HMENU((index + 100) as *mut c_void)),
                    None,
                    None,
                )
            }
            .map_err(|cause| ReportSurfaceError::new("report_control_create", cause))?;
            let registration = Registration::new(&self.callback);
            self.peers[index] = Some(Peer { hwnd, registration });
            if index == 1 {
                // SAFETY: Bound the retained read-only EDIT; EM_SETLIMITTEXT has no return value.
                let _no_return = unsafe {
                    SendMessageW(
                        hwnd,
                        EM_SETLIMITTEXT,
                        Some(WPARAM(NativeReportView::MAX_BYTES * 2)),
                        None,
                    )
                };
            }
            text.verify(hwnd, &self.lease)?;
            let mut children = self.callback.children.get();
            children[index] = hwnd;
            self.callback.children.set(children);
            // SAFETY: The newly adopted child is retained before installing its same typed callback.
            unsafe {
                install(
                    hwnd,
                    Some(procedure),
                    &self.peers[index]
                        .as_ref()
                        .ok_or_else(|| ReportSurfaceError::rejected("report_control_adoption"))?
                        .registration,
                )
            }?;
        }
        self.layout()?;
        if !view.restart().allows_restart() {
            self.status(labels.restart_unavailable)?;
        }
        self.enable(false, view.restart())?;
        Ok(())
    }
    pub(super) fn layout(&mut self) -> Result<(), ReportSurfaceError> {
        // SAFETY: Query the exact live parent on its creating thread.
        let dpi = unsafe { GetDpiForWindow(self.callback.parent) };
        if let Some(font) = self.fonts.prepare(dpi)? {
            for peer in self.peers.iter().flatten() {
                // SAFETY: Both old and new fonts stay owned until every child switches or destroys.
                let _no_return = unsafe {
                    SendMessageW(
                        peer.hwnd,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    )
                };
                // SAFETY: Check the actual font adopted by this retained stock control.
                if unsafe { SendMessageW(peer.hwnd, WM_GETFONT, None, None) }.0 != font.0 as isize {
                    return Err(ReportSurfaceError::rejected("report_control_font"));
                }
            }
            self.fonts.commit()?;
        }
        let size = self.window.inner_size();
        let scale = self.window.scale_factor();
        let width = (f64::from(size.width) / scale).max(360.0);
        let height = (f64::from(size.height) / scale).max(280.0);
        let two_rows = width < 760.0;
        let bottom = if two_rows { 86.0 } else { 44.0 };
        let text_height = (height - 164.0 - bottom).max(24.0);
        let button_width = if two_rows { (width - 48.0) / 2.0 } else { (width - 80.0) / 4.0 };
        let mut bounds = [
            [16.0, 10.0, width - 32.0, 34.0],
            [16.0, 48.0, width - 32.0, text_height],
            [16.0, 54.0 + text_height, width - 32.0, 26.0],
            [16.0, 82.0 + text_height, width - 32.0, 64.0],
            [0.0; 4],
            [0.0; 4],
            [0.0; 4],
            [0.0; 4],
        ];
        for index in 0..4 {
            let columns = if two_rows { 2 } else { 4 };
            bounds[index + 4] = [
                16.0 + (index % columns) as f64 * (button_width + 16.0),
                height - bottom + (index / columns) as f64 * 42.0,
                button_width,
                32.0,
            ];
        }
        for (peer, bounds) in self.peers.iter().zip(bounds) {
            if let Some(peer) = peer {
                let [x, y, w, h] = bounds.map(|value| (value * scale).round() as i32);
                // SAFETY: Move only these live retained children on their creating UI thread.
                unsafe { SetWindowPos(peer.hwnd, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER) }
                    .map_err(|cause| ReportSurfaceError::new("report_control_layout", cause))?;
            }
        }
        Ok(())
    }
    pub(super) fn enable(
        &self,
        enabled: bool,
        restart: ReportRestartAvailability,
    ) -> Result<(), ReportSurfaceError> {
        self.callback.enabled.set(false);
        self.callback.restart.set(restart);
        for (index, peer) in self
            .peers
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(i, p)| p.as_ref().map(|peer| (i, peer)))
        {
            let enabled = enabled && (index != 7 || restart.allows_restart());
            // SAFETY: Actual current child is retained on its UI thread. BOOL means previous state.
            unsafe {
                let _previous_enablement = EnableWindow(peer.hwnd, enabled);
            }
            // SAFETY: Verify resulting enablement instead of treating previous-state BOOL as success.
            if unsafe { IsWindowEnabled(peer.hwnd) }.as_bool() != enabled {
                return Err(ReportSurfaceError::rejected("report_control_enablement"));
            }
        }
        self.callback.enabled.set(enabled);
        Ok(())
    }
    pub(super) fn replace(&self, view: &NativeReportView) -> Result<(), ReportSurfaceError> {
        self.enable(false, view.restart())?;
        let text = Text::new(view.text(), &self.lease)?;
        let children = self.callback.children.get();
        // SAFETY: The bounded temporary text is synchronously copied by the retained EDIT.
        unsafe { SetWindowTextW(children[1], PCWSTR(text.units.as_ptr())) }
            .map_err(|cause| ReportSurfaceError::new("report_view_native_replace", cause))?;
        text.verify(children[1], &self.lease)?;
        // SAFETY: Reset sensitive-memory consent on every actual replacement. BM_SETCHECK returns zero.
        let check = unsafe {
            SendMessageW(children[2], BM_SETCHECK, Some(WPARAM(BST_UNCHECKED.0 as usize)), None)
        };
        if check.0 != 0 {
            return Err(ReportSurfaceError::rejected("report_consent_reset"));
        }
        self.callback.binding.set(view.binding());
        Ok(())
    }
    pub(super) fn status(&self, text: &str) -> Result<(), ReportSurfaceError> {
        let text = Text::new(text, &self.lease)?;
        // SAFETY: Retained localized native status label synchronously copies bounded UTF16.
        unsafe { SetWindowTextW(self.callback.children.get()[0], PCWSTR(text.units.as_ptr())) }
            .map_err(|cause| ReportSurfaceError::new("report_status_native", cause))
    }
    pub(super) fn retire(&mut self) -> Result<(), ReportSurfaceError> {
        self.callback.enabled.set(false);
        self.clipboard.retire()?;
        for slot in &mut self.peers {
            if let Some(peer) = slot {
                let was_installed = peer.registration.installed();
                // SAFETY: Retained live child is destroyed only on its creating thread.
                unsafe { DestroyWindow(peer.hwnd) }
                    .map_err(|cause| ReportSurfaceError::new("report_control_destroy", cause))?;
                if peer.registration.installed()
                    || (was_installed && !peer.registration.destroyed())
                {
                    return Err(ReportSurfaceError::rejected("report_control_callback_retirement"));
                }
                *slot = None;
            }
        }
        if self.parent.installed() {
            remove(self.callback.parent, Some(procedure), &self.parent)?;
        }
        self.callback.children.set([HWND::default(); 8]);
        self.fonts.retire_after_children()?;
        Ok(())
    }
}
