// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one asynchronous document-start script obligation through actual native completion.
use super::queue;
use crate::{DesktopError, main_events::Events};
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HRESULT, HSTRING, PCWSTR, implement};
struct State {
    pending: bool,
    completed: bool,
    identifier: String,
    failure: Option<DesktopError>,
}
pub(super) struct Registration {
    state: Rc<Shared>,
    script: Option<String>,
    submitted: bool,
    uncertain: bool,
}
struct Shared {
    state: RefCell<State>,
    events: Rc<Events>,
    _memory: MemoryClaim,
}
impl Registration {
    pub(super) fn new(events: Rc<Events>, memory: &MemoryLease) -> Result<Self, DesktopError> {
        let claim =
            memory.claim(4096).map_err(|cause| DesktopError::new("claim_session_script", cause))?;
        let mut identifier = String::new();
        identifier
            .try_reserve_exact(128)
            .map_err(|cause| DesktopError::new("allocate_session_script", cause))?;
        Ok(Self {
            state: Rc::new(Shared {
                state: RefCell::new(State {
                    pending: false,
                    completed: false,
                    identifier,
                    failure: None,
                }),
                events,
                _memory: claim,
            }),
            script: None,
            submitted: false,
            uncertain: false,
        })
    }
    pub(super) fn submit(
        &mut self,
        core: &ICoreWebView2,
        source: &str,
    ) -> Result<(), DesktopError> {
        if self.submitted || self.state.events.in_callback() || source.len() > 64 * 1024 {
            return Err(DesktopError::message(
                "install_session_script",
                "script admission refused",
            ));
        }
        self.submitted = true;
        self.state.state.borrow_mut().pending = true;
        let handler: ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler =
            Completed { shared: self.state.clone() }.into();
        let source = HSTRING::from(source);
        // SAFETY: UI-affine core/source/handler stay live; pending custody is recorded before submission.
        if let Err(cause) = unsafe { core.AddScriptToExecuteOnDocumentCreated(&source, &handler) } {
            self.uncertain = true;
            return Err(DesktopError::new("install_session_script", cause));
        }
        Ok(())
    }
    pub(super) fn submitted(&self) -> bool {
        self.submitted
    }
    pub(super) fn pending(&self) -> bool {
        self.state.state.borrow().pending
    }
    pub(super) fn completed(&self) -> bool {
        self.state.state.borrow().completed
    }
    pub(super) fn observe(&mut self) -> Result<bool, DesktopError> {
        let mut state = self.state.state.borrow_mut();
        if !state.completed {
            return Ok(self.script.is_some());
        }
        state.completed = false;
        if !state.identifier.is_empty() {
            self.script = Some(std::mem::take(&mut state.identifier));
        }
        if let Some(cause) = state.failure.take() {
            self.uncertain = self.script.is_none();
            return Err(cause);
        }
        if self.script.is_none() {
            self.uncertain = true;
            return Err(DesktopError::message(
                "complete_session_script",
                "native script identifier missing",
            ));
        }
        self.uncertain = false;
        Ok(true)
    }
    pub(super) fn retire(&mut self, core: &ICoreWebView2) -> Result<bool, DesktopError> {
        if self.pending() {
            return Ok(false);
        }
        self.observe()?;
        if self.uncertain {
            return Ok(false);
        }
        if let Some(script) = &self.script {
            let script = HSTRING::from(script);
            // SAFETY: This exact observed native identifier belongs to the retained creating-thread core.
            unsafe { core.RemoveScriptToExecuteOnDocumentCreated(&script) }
                .map_err(|cause| DesktopError::new("remove_session_script", cause))?;
            self.script = None;
        }
        Ok(Rc::strong_count(&self.state) == 1)
    }
}
#[implement(ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler)]
struct Completed {
    shared: Rc<Shared>,
}
impl ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler_Impl for Completed_Impl {
    fn Invoke(&self, code: HRESULT, result: &PCWSTR) -> windows::core::Result<()> {
        let _callback = self.shared.events.callback();
        let mut state = self.shared.state.borrow_mut();
        if !state.pending || state.completed {
            DesktopError::retain_first(
                &mut state.failure,
                DesktopError::message(
                    "complete_session_script",
                    "duplicate or foreign native callback",
                ),
            );
        } else {
            if !result.is_null() {
                let copied = queue::copy(&mut state.identifier, 128, |index| {
                    // SAFETY: This callback owns the borrowed NUL-terminated native result until return.
                    unsafe { result.as_ptr().add(index).read() }
                });
                if let Err(cause) = copied {
                    DesktopError::retain_first(&mut state.failure, cause);
                }
            }
            if let Err(cause) = code.ok() {
                DesktopError::retain_first(
                    &mut state.failure,
                    DesktopError::new("complete_session_script", cause),
                );
            } else if result.is_null() {
                DesktopError::retain_first(
                    &mut state.failure,
                    DesktopError::message("complete_session_script", "null native identifier"),
                );
            }
        }
        state.pending = false;
        state.completed = true;
        drop(state);
        self.shared.events.wake();
        Ok(())
    }
}
#[cfg(test)]
mod tests;
