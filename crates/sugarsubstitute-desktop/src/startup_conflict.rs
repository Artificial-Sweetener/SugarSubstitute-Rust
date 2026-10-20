// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep conflict decisions responsive while the application owns asynchronous native work.
use crate::DesktopError;
use std::sync::{
    Arc,
    atomic::{AtomicIsize, Ordering},
};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};
use sugarsubstitute_contracts::localization::conflict::ConflictMessages;
use windows::{
    Win32::{
        Foundation::*,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::{HRESULT, PCWSTR},
};

use sugarsubstitute_contracts::instance_conflict::{ConflictAction, ConflictController};
struct Dialog<'a> {
    controller: &'a mut dyn ConflictController,
    text: String,
    done: bool,
    wake: Arc<DialogWake>,
}
struct DialogWake(AtomicIsize);
impl CompletionWake for DialogWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let raw = self.0.load(Ordering::Acquire);
        if raw != 0 {
            // SAFETY: The modal lifetime retains this HWND; WM_NULL carries no borrowed data.
            unsafe { PostMessageW(Some(HWND(raw as *mut _)), WM_NULL, WPARAM(0), LPARAM(0)) }
                .map_err(|cause| {
                    ExecutionError::new(
                        sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                        Box::new(cause),
                    )
                })?;
        }
        Ok(())
    }
}
/// Run a timer-driven native modal surface until the service has actually retired its work.
/// # Errors
/// Native dialog failure is returned; the caller must still retire its service separately.
pub fn show_startup_conflict(
    title: &str,
    labels: &ConflictMessages,
    controller: &mut dyn ConflictController,
) -> Result<(), DesktopError> {
    let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let title = wide(title);
    let heading = wide(labels.title);
    let initial = wide(labels.checking);
    let close = wide(labels.close);
    let force = wide(labels.force);
    let cancel = wide(labels.cancel);
    let buttons = [
        TASKDIALOG_BUTTON { nButtonID: 100, pszButtonText: PCWSTR(close.as_ptr()) },
        TASKDIALOG_BUTTON { nButtonID: 101, pszButtonText: PCWSTR(force.as_ptr()) },
        TASKDIALOG_BUTTON { nButtonID: 102, pszButtonText: PCWSTR(cancel.as_ptr()) },
    ];
    let mut dialog = Dialog {
        controller,
        text: String::new(),
        done: false,
        wake: Arc::new(DialogWake(AtomicIsize::new(0))),
    };
    let config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_CALLBACK_TIMER | TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(heading.as_ptr()),
        pszContent: PCWSTR(initial.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 102,
        pfCallback: Some(callback),
        lpCallbackData: (&raw mut dialog) as isize,
        ..Default::default()
    };
    // SAFETY: All strings, buttons and callback state remain owned through this synchronous modal call.
    let result = unsafe { TaskDialogIndirect(&config, None, None, None) }
        .map_err(|cause| DesktopError::new("profile_conflict_dialog", cause));
    dialog.wake.0.store(0, Ordering::Release);
    result
}
unsafe extern "system" fn callback(
    hwnd: HWND,
    notice: TASKDIALOG_NOTIFICATIONS,
    button: WPARAM,
    _: LPARAM,
    data: isize,
) -> HRESULT {
    // SAFETY: TaskDialog borrows this live state synchronously on the calling UI thread.
    let state = unsafe { &mut *(data as *mut Dialog<'_>) };
    if notice == TDN_CREATED {
        state.wake.0.store(hwnd.0 as isize, Ordering::Release);
        state.controller.start(state.wake.clone());
    }
    if notice == TDN_DESTROYED {
        state.wake.0.store(0, Ordering::Release);
    }
    if notice == TDN_BUTTON_CLICKED {
        if state.done {
            return S_OK;
        }
        state.controller.choose(match button.0 {
            100 => ConflictAction::Close,
            101 => ConflictAction::Force,
            _ => ConflictAction::Cancel,
        });
        return S_FALSE;
    }
    if notice == TDN_CREATED || notice == TDN_TIMER {
        let view = state.controller.poll();
        let text = if state.text != view.text {
            let text: Vec<_> = view.text.encode_utf16().chain(Some(0)).collect();
            state.text = view.text;
            Some(text)
        } else {
            None
        };
        state.done = view.done;
        // End the exclusive callback-state borrow before messages can reenter this callback.
        // SAFETY: TaskDialog owns hwnd; synchronous messages copy each string before return.
        unsafe {
            SendMessageW(
                hwnd,
                TDM_ENABLE_BUTTON.0 as u32,
                Some(WPARAM(100)),
                Some(LPARAM(isize::from(view.close_enabled))),
            );
            SendMessageW(
                hwnd,
                TDM_ENABLE_BUTTON.0 as u32,
                Some(WPARAM(101)),
                Some(LPARAM(isize::from(view.force_enabled))),
            );
            if let Some(text) = text {
                SendMessageW(
                    hwnd,
                    TDM_SET_ELEMENT_TEXT.0 as u32,
                    Some(WPARAM(TDE_CONTENT.0 as usize)),
                    Some(LPARAM(text.as_ptr() as isize)),
                );
            }
            if view.done {
                SendMessageW(hwnd, TDM_CLICK_BUTTON.0 as u32, Some(WPARAM(102)), Some(LPARAM(0)));
            }
        }
    }
    S_OK
}
