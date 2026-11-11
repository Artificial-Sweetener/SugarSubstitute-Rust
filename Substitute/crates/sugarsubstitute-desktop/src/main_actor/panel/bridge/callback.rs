// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded CDP callback results independently of ordinary event admission.

use super::ledger::Action;
use crate::{
    DesktopError,
    main_events::{Events, PAYLOAD_BYTES, copy_utf16},
};
use std::{cell::RefCell, rc::Rc};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HRESULT, HSTRING, PCWSTR, implement};

struct State {
    pending: Option<Action>,
    completed: Option<(Action, Result<(), DesktopError>)>,
    payload: String,
}

/// One callback obligation and its preallocated result remain UI-affine.
pub(super) struct Shared {
    state: RefCell<State>,
    events: Rc<Events>,
}

impl Shared {
    /// Allocate callback storage before any native registration.
    pub(super) fn new(events: Rc<Events>) -> Result<Rc<Self>, DesktopError> {
        let mut payload = String::new();
        payload
            .try_reserve_exact(PAYLOAD_BYTES)
            .map_err(|cause| DesktopError::new("allocate_panel_bridge_callback", cause))?;
        if payload.capacity() != PAYLOAD_BYTES {
            return Err(DesktopError::message(
                "allocate_panel_bridge_callback",
                "unexpected capacity",
            ));
        }
        Ok(Rc::new(Self {
            state: RefCell::new(State { pending: None, completed: None, payload }),
            events,
        }))
    }

    /// Report whether native completion is still outstanding.
    pub(super) fn waiting(&self) -> bool {
        self.state.borrow().pending.is_some()
    }

    /// Report an actual callback result awaiting normal-loop processing.
    pub(super) fn completed(&self) -> bool {
        self.state.borrow().completed.is_some()
    }

    /// Decode a completed command without holding this borrow across a native effect.
    pub(super) fn take(&self) -> Option<(Action, Result<Option<String>, DesktopError>)> {
        let mut state = self.state.borrow_mut();
        let (action, result) = state.completed.take()?;
        let decoded = result.and_then(|()| decode(action, &state.payload));
        state.payload.clear();
        Some((action, decoded))
    }
}

/// Keep an exact registration callback alive even if ordinary event admission is closed.
#[implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct Completed {
    shared: Rc<Shared>,
    action: Action,
}

impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for Completed_Impl {
    fn Invoke(&self, code: HRESULT, value: &PCWSTR) -> windows::core::Result<()> {
        let _callback = self.shared.events.callback();
        let mut state = self.shared.state.borrow_mut();
        if state.pending != Some(self.action) || state.completed.is_some() {
            drop(state);
            self.shared.events.fail(DesktopError::message(
                "complete_panel_bridge",
                "duplicate or foreign registration callback",
            ));
            return Ok(());
        }
        let result = if let Err(cause) = code.ok() {
            Err(DesktopError::new("complete_panel_bridge", cause))
        } else if value.is_null() {
            Err(DesktopError::new(
                "complete_panel_bridge",
                windows::core::Error::from(windows::Win32::Foundation::E_POINTER),
            ))
        } else {
            copy_utf16(&mut state.payload, |index| {
                // SAFETY: WebView2 retains this borrowed NUL-terminated result throughout Invoke.
                unsafe { value.as_ptr().add(index).read() }
            })
        };
        state.pending = None;
        state.completed = Some((self.action, result));
        drop(state);
        self.shared.events.wake();
        Ok(())
    }
}

/// Arm callback custody before a native submission that can synchronously invoke it.
pub(super) fn submit(
    core: &ICoreWebView2,
    shared: &Rc<Shared>,
    action: Action,
    method: &str,
    parameters: &str,
) -> Result<(), DesktopError> {
    if shared.events.in_callback() || method.len() > 128 || parameters.len() > 8192 {
        return Err(DesktopError::message("submit_panel_bridge", "invalid native submission"));
    }
    {
        let mut state = shared.state.borrow_mut();
        if state.pending.is_some() || state.completed.is_some() {
            return Err(DesktopError::message("submit_panel_bridge", "callback remains owned"));
        }
        state.payload.clear();
        state.pending = Some(action);
    }
    let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler =
        Completed { shared: shared.clone(), action }.into();
    let method = HSTRING::from(method);
    let parameters = HSTRING::from(parameters);
    // SAFETY: The creating thread retains core and handler; the callback obligation was armed first.
    unsafe { core.CallDevToolsProtocolMethod(&method, &parameters, &handler) }
        .map_err(|cause| DesktopError::new("submit_panel_bridge", cause))
}

/// Validate only the fixed native result shape, keeping the returned script id bounded.
fn decode(action: Action, payload: &str) -> Result<Option<String>, DesktopError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Empty {}
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Script {
        identifier: String,
    }
    if matches!(action, Action::AddScript(_)) {
        let value: Script = serde_json::from_str(payload)
            .map_err(|cause| DesktopError::new("decode_panel_script", cause))?;
        if value.identifier.is_empty()
            || value.identifier.len() > 128
            || value.identifier.chars().any(char::is_control)
        {
            return Err(DesktopError::message("decode_panel_script", "invalid script identifier"));
        }
        return Ok(Some(value.identifier.into_boxed_str().into_string()));
    }
    let _: Empty = serde_json::from_str(payload)
        .map_err(|cause| DesktopError::new("decode_panel_bridge", cause))?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_completion_keeps_only_a_bounded_exact_script_identifier() {
        assert_eq!(
            decode(Action::AddScript(0), r#"{"identifier":"7"}"#).ok().flatten().as_deref(),
            Some("7")
        );
        assert!(decode(Action::AddScript(0), r#"{"identifier":""}"#).is_err());
        assert!(decode(Action::AddScript(0), r#"{"identifier":"7","extra":true}"#).is_err());
        assert!(decode(Action::AddBinding, "{}").is_ok());
        assert!(decode(Action::RemoveBinding, r#"{"extra":true}"#).is_err());
        assert!(
            decode(Action::AddScript(0), &format!(r#"{{"identifier":"{}"}}"#, "x".repeat(129)))
                .is_err()
        );
    }
}
