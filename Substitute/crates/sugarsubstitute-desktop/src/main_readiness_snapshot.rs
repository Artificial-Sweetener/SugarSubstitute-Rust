// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only stored readiness wait facts; no evaluation, clock or external provider runs.
use super::MainReadiness;
/// Closed owner stages, independent of native process or readiness authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Wait {
    Fault,
    Ready,
    Failed,
    Setup,
    Navigation,
    Root,
    Context,
    Document,
    Probe,
    Barrier,
}
/// Borrowed issued operation identity; payloads, frame/context strings and paths remain private.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) wait: Wait,
    pub(crate) operation: Option<u64>,
}
impl MainReadiness {
    /// Borrow this exact owner's stored state without modifying admission or pending custody.
    pub(crate) fn wait_snapshot(&self) -> Snapshot {
        let wait = if self.fault.is_some() {
            Wait::Fault
        } else if self.completed {
            if self.positive { Wait::Ready } else { Wait::Failed }
        } else if let Some(pending) = &self.pending {
            if pending.receipt.is_some() { Wait::Barrier } else { Wait::Probe }
        } else if !self.started {
            Wait::Setup
        } else if !self.loaded {
            Wait::Navigation
        } else if self.root.is_none() {
            Wait::Root
        } else if self.default_context.is_none() {
            Wait::Context
        } else {
            Wait::Document
        };
        Snapshot { wait, operation: self.pending.as_ref().map(|pending| pending.target.operation) }
    }
}
