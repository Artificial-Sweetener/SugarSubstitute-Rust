// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify a real UIA client on an execution-owned MTA while its native STA pumps messages.
use crate::windows_text::Engine;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::{Com::*, Threading::GetCurrentThreadId},
        UI::{Accessibility::*, WindowsAndMessaging::*},
    },
    core::Interface,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

struct ClockPort;
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake(u32);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        // SAFETY: The fixture retains this existing native queue through observed job completion.
        unsafe { PostThreadMessageW(self.0, WM_APP, WPARAM(0), LPARAM(0)) }
            .map_err(|cause| ExecutionError::new(Operation::NativeLifecycle, Box::new(cause)))
    }
}
#[derive(Default)]
struct State {
    ready: bool,
    released: bool,
}
#[derive(Default)]
struct Gate {
    state: Mutex<State>,
    changed: Condvar,
}
impl Gate {
    fn release(&self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(error) => error.into_inner(),
        };
        state.released = true;
        self.changed.notify_all();
    }
    fn ready(&self) -> Result<bool, String> {
        Ok(self.state.lock().map_err(|e| e.to_string())?.ready)
    }
    fn wait_for_close(&self, wake: &Wake) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.ready = true;
        wake.wake().map_err(|e| e.to_string())?;
        state =
            self.changed.wait_while(state, |state| !state.released).map_err(|e| e.to_string())?;
        drop(state);
        Ok(())
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// No COM interface crosses the job boundary; the HWND stays retained until the client retires.
pub(super) fn qualify(
    engine: &Engine,
    source: &str,
    readonly: bool,
    sibling: Option<(&Engine, String)>,
) -> TestResult {
    // SAFETY: The caller owns a live HWND and therefore an initialized native message queue.
    let wake = Arc::new(Wake(unsafe { GetCurrentThreadId() }));
    let mut pending = MSG::default();
    // SAFETY: Establish the owning STA as a pumping GUI thread before admitting a UIA client.
    unsafe {
        let _pending = PeekMessageW(&mut pending, None, 0, 0, PM_NOREMOVE);
    }
    let mut owner = ExecutionOwner::new(Arc::new(ClockPort), wake.clone())?;
    let gate = Arc::new(Gate::default());
    let release = Release(gate.clone());
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let hwnd = engine.state.hwnd.0 as usize;
    let expected = source.to_owned();
    let sibling = sibling.map(|(engine, text)| (engine.state.hwnd.0 as usize, text));
    let worker_gate = gate.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1_000_000_000)),
            reservation_bytes: 4096,
            replacement: None,
        },
        move |_| observe(hwnd, &expected, readonly, sibling.as_ref(), &worker_gate, &wake),
    )?;
    let mut closed = false;
    let result = loop {
        let _completions = client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if let Some(completion) = job.try_take_completion() {
            break match completion.outcome {
                JobOutcome::Success(()) => Ok(()),
                other => Err(format!("UI Automation client terminal: {other:?}").into()),
            };
        }
        if !closed && gate.ready()? {
            if !engine.accessibility.borrow().as_ref().ok_or("UIA owner absent")?.published.get() {
                return Err("UIA client attached but publication was not recorded".into());
            }
            engine.close()?;
            closed = true;
            gate.release();
        }
        let mut message = MSG::default();
        // SAFETY: Pump the fixture's STA, including COM calls marshaled by the actual UIA client.
        let received = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if received <= 0 {
            return Err(format!("UI Automation fixture message queue stopped: {received}").into());
        }
        // SAFETY: Dispatch only native messages acquired from this fixture's owning thread.
        unsafe {
            let _translated = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };
    owner.request_shutdown(Moment::from_nanos(1_000_000_000));
    drop(job);
    drop(scope);
    drop(client);
    drop(release);
    drop(owner);
    result
}

fn observe(
    hwnd: usize,
    expected: &str,
    readonly: bool,
    sibling: Option<&(usize, String)>,
    gate: &Gate,
    wake: &Wake,
) -> Result<(), String> {
    // SAFETY: This execution worker owns no windows and balances its acquired MTA reference.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|e| format!("MTA: {e:?}"))?;
    let result = observe_initialized(HWND(hwnd as *mut _), expected, readonly, sibling, gate, wake);
    // SAFETY: Every client interface in observe_initialized retired before this exact COM balance.
    unsafe { CoUninitialize() };
    result
}

fn observe_initialized(
    hwnd: HWND,
    expected: &str,
    readonly: bool,
    sibling: Option<&(usize, String)>,
    gate: &Gate,
    wake: &Wake,
) -> Result<(), String> {
    let mut stage = "create";
    let client = (|| -> windows::core::Result<(IUIAutomation, IUIAutomationValuePattern, Option<IUIAutomationValuePattern>)> {
        // SAFETY: All client interfaces remain affine to this initialized MTA worker.
        unsafe {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
            stage = "settings";
            let settings: IUIAutomation2 = automation.cast()?;
            settings.SetConnectionTimeout(3000)?;
            settings.SetTransactionTimeout(3000)?;
            stage = "parent";
            let parent = GetParent(hwnd)?;
            let _parent = automation.ElementFromHandle(parent)?;
            stage = "element";
            let root = automation.ElementFromHandle(hwnd)?;
            stage = "child";
            let text = root.FindFirst(TreeScope_Children, &automation.CreateTrueCondition()?)?;
            stage = "value pattern";
            let value =
                text.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?;
            stage = "sibling";
            let sibling_value = if let Some((handle, _)) = sibling {
                let other_root = automation.ElementFromHandle(HWND(*handle as *mut _))?;
                let other = other_root.FindFirst(TreeScope_Children, &automation.CreateTrueCondition()?)?;
                if automation.CompareElements(&text, &other)?.as_bool() {
                    return Err(windows::Win32::Foundation::E_UNEXPECTED.into());
                }
                Some(other.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?)
            } else { None };
            Ok((automation, value, sibling_value))
        }
    })()
    .map_err(|e| format!("UIA client attach {stage}: {e:?}; window {hwnd:?}"));
    let client = client?;
    let (_automation, value, sibling_value) = client;
    // SAFETY: These are supported UIA client calls; COM marshals them to the pumping provider STA.
    let (actual, actual_readonly) = unsafe { (value.CurrentValue(), value.CurrentIsReadOnly()) };
    let actual = actual.map_err(|e| format!("UIA value: {e:?}"))?.to_string();
    let actual_readonly = actual_readonly.map_err(|e| format!("UIA readonly: {e:?}"))?.as_bool();
    if actual != expected || actual_readonly != readonly {
        return Err(format!(
            "UIA client observed text {actual:?} readonly {actual_readonly}; expected {expected:?} readonly {readonly}"
        ));
    }
    gate.wait_for_close(wake)?;
    // SAFETY: The retained client pattern must reject after actual provider disconnection.
    match unsafe { value.CurrentValue() } {
        Err(error) if error.code().0 == UIA_E_ELEMENTNOTAVAILABLE as i32 => {}
        other => return Err(format!("UIA retained client did not retire: {other:?}")),
    }
    if let (Some(value), Some((_, expected))) = (sibling_value, sibling) {
        // SAFETY: This independently retained sibling must remain readable after the first closes.
        let observed = unsafe { value.CurrentValue() }
            .map_err(|cause| format!("UIA sibling retired: {cause:?}"))?;
        if observed != expected.as_str() {
            return Err("UIA sibling text changed after another control closed".into());
        }
    }
    Ok(())
}
