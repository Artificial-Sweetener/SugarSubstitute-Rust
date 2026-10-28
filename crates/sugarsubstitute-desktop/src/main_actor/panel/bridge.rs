// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Install and retire one public panel binding on the original WebView apartment.

mod callback;
mod ledger;

use crate::{DesktopError, main_events::Events};
use ledger::{Action, Ledger, Next};
use std::rc::Rc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;

const SCRIPT: &str = include_str!("bridge.js");

/// Own every setup callback, script identifier and registration until checked removal.
pub(in crate::main_actor) struct Bridge {
    core: ICoreWebView2,
    shared: Rc<callback::Shared>,
    ledger: Ledger,
    script: Option<String>,
    _claim: MemoryClaim,
}

impl Bridge {
    /// Claim all callback/control storage before submitting a native registration.
    pub(in crate::main_actor) fn new(
        core: ICoreWebView2,
        events: Rc<Events>,
        lease: &MemoryLease,
    ) -> Result<Self, DesktopError> {
        let claim = lease
            .claim(64 * 1024)
            .map_err(|cause| DesktopError::new("claim_panel_bridge", cause))?;
        Ok(Self {
            core,
            shared: callback::Shared::new(events)?,
            ledger: Ledger::new(),
            script: None,
            _claim: claim,
        })
    }

    /// Identify actual completed callbacks or the next nonblocking setup effect.
    pub(in crate::main_actor) fn needs_progress(&self) -> bool {
        self.shared.completed()
            || (!self.shared.waiting() && matches!(self.ledger.next(), Next::Effect(_)))
    }

    /// Complete installation before navigating the actual Comfy document.
    pub(in crate::main_actor) fn install_step(&mut self) -> Result<bool, DesktopError> {
        self.complete_callback()?;
        match self.ledger.next() {
            Next::Effect(action @ (Action::AddBinding | Action::AddScript)) => {
                self.submit(action)?;
                Ok(false)
            }
            Next::Await => Ok(false),
            Next::Ready => Ok(true),
            _ => Err(DesktopError::message(
                "install_panel_bridge",
                "registration is not installable",
            )),
        }
    }

    /// Fence setup and remove the exact script before its binding; wait for actual callbacks.
    pub(in crate::main_actor) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        self.ledger.close();
        self.complete_callback()?;
        match self.ledger.next() {
            Next::Effect(action @ (Action::RemoveScript | Action::RemoveBinding)) => {
                self.submit(action)?;
                Ok(false)
            }
            Next::Await => Ok(false),
            Next::Retired => Ok(true),
            Next::Uncertain => Err(DesktopError::message(
                "retire_panel_bridge",
                "script registration remains uncertain",
            )),
            _ => Err(DesktopError::message("retire_panel_bridge", "registration cannot retire")),
        }
    }

    /// Transfer one callback result into its matching ledger transition.
    fn complete_callback(&mut self) -> Result<(), DesktopError> {
        let Some((action, result)) = self.shared.take() else { return Ok(()) };
        let success = result.is_ok();
        if !self.ledger.complete(action, success) {
            return Err(DesktopError::message(
                "complete_panel_bridge",
                "callback is stale or foreign",
            ));
        }
        let identifier = result?;
        match action {
            Action::AddScript => self.script = identifier,
            Action::RemoveScript => self.script = None,
            Action::AddBinding | Action::RemoveBinding => {}
        }
        Ok(())
    }

    /// Submit one fixed CDP operation after recording its potential native effect.
    fn submit(&mut self, action: Action) -> Result<(), DesktopError> {
        let (method, parameters) = match action {
            Action::AddBinding => (
                "Runtime.addBinding",
                serde_json::json!({"name": crate::main_panel_ingress::BINDING_NAME}),
            ),
            Action::AddScript => {
                ("Page.addScriptToEvaluateOnNewDocument", serde_json::json!({"source": SCRIPT}))
            }
            Action::RemoveScript => (
                "Page.removeScriptToEvaluateOnNewDocument",
                serde_json::json!({"identifier": self.script.as_deref().ok_or_else(|| DesktopError::message("retire_panel_bridge", "script identifier absent"))?}),
            ),
            Action::RemoveBinding => (
                "Runtime.removeBinding",
                serde_json::json!({"name": crate::main_panel_ingress::BINDING_NAME}),
            ),
        };
        let parameters = serde_json::to_string(&parameters)
            .map_err(|cause| DesktopError::new("encode_panel_bridge", cause))?;
        if !self.ledger.begin(action) {
            return Err(DesktopError::message("submit_panel_bridge", "effect is out of order"));
        }
        callback::submit(&self.core, &self.shared, action, method, &parameters)
    }
}
