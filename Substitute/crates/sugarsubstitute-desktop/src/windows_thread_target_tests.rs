// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HWND_MESSAGE, MSG, PM_REMOVE,
    PeekMessageW, RegisterClassW, SendMessageW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WNDCLASSW,
};
use windows::core::w;

static PAINTS: AtomicUsize = AtomicUsize::new(0);

struct Still;
impl Clock for Still {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Ignore;
impl CompletionWake for Ignore {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}

unsafe extern "system" fn base(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        PAINTS.fetch_add(1, Ordering::SeqCst);
        return LRESULT(0);
    }
    // SAFETY: Default handling of the test window's own messages.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[test]
fn a_paint_sent_from_another_thread_never_reaches_the_event_loop_window()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(sent_paint(WM_PAINT, 1));
    assert!(!sent_paint(WM_PAINT, ISMEX_NOSEND));
    assert!(!sent_paint(WM_NCDESTROY, 1));
    let class = WNDCLASSW {
        lpfnWndProc: Some(base),
        lpszClassName: w!("SugarPaintGuardTest"),
        ..Default::default()
    };
    // SAFETY: Registers a class whose procedure is the static test procedure.
    unsafe { RegisterClassW(&class) };
    // SAFETY: A message-only window owned by this test thread.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("SugarPaintGuardTest"),
            None,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )?
    };
    guard(hwnd);
    let target = hwnd.0 as isize;
    let mut owner = ExecutionOwner::new(Arc::new(Still), Arc::new(Ignore))?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 4096,
        replacement: None,
    };
    // An execution-owned worker sends synchronously from another thread.
    let mut sender = client.submit_blocking(&scope, spec, move |_| {
        // SAFETY: A synchronous cross-thread send to the live test window.
        unsafe { SendMessageW(HWND(target as *mut _), WM_PAINT, None, None) };
        Ok::<(), std::io::Error>(())
    })?;
    // The send is delivered while this thread pumps; the loop ends once it was answered.
    loop {
        let mut message = MSG::default();
        // SAFETY: Pumps this thread's queue, which delivers the pending sent message.
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            // SAFETY: Standard dispatch of a message this thread retrieved.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if sender.try_take_completion().is_some() {
            break;
        }
    }
    drop(sender);
    drop(scope);
    owner.request_shutdown(Moment::from_nanos(0));
    while !owner.poll_shutdown()?.settled {}
    owner.finish_shutdown()?;
    assert_eq!(PAINTS.load(Ordering::SeqCst), 0);
    // SAFETY: A same-thread send is a direct call, not a sent message.
    unsafe { SendMessageW(hwnd, WM_PAINT, None, None) };
    assert_eq!(PAINTS.load(Ordering::SeqCst), 1);
    // SAFETY: Destroys the test window on its own thread.
    unsafe { DestroyWindow(hwnd)? };
    Ok(())
}
