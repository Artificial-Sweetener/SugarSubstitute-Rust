// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the single original frame separately from finite worker results.
use super::*;
use std::sync::{Arc, Mutex};
use sugarsubstitute_application::transcript::TranscriptStore;
use sugarsubstitute_contracts::execution::JobContext;
pub(super) struct State {
    pub(super) transcript: TranscriptStore,
    pub(super) frame: Option<ReceivedLifecycleFrame>,
}
#[derive(Clone)]
pub(super) struct Source {
    pub(super) state: Arc<Mutex<State>>,
    pub(super) custody: snapshot::SourceCustody,
    owner: sugarsubstitute_contracts::execution::ExecutionId,
    scope: sugarsubstitute_contracts::execution::ScopeId,
}
impl Source {
    pub(super) fn check(&self, context: &JobContext) -> Result<(), OutputError> {
        if context.owner_id() != self.owner || context.scope_id() != self.scope {
            return Err(OutputError::Identity);
        }
        worker::check(context)
    }
    pub(super) fn new(binding: OutputBinding, context: &JobContext) -> Result<Self, OutputError> {
        worker::check(context)?;
        let claim = context.memory().claim(SOURCE_BYTES).map_err(OutputError::Claim)?;
        let custody = snapshot::SourceCustody::new(claim, context.memory().clone());
        let state = Arc::new(Mutex::new(State {
            transcript: TranscriptStore::new(binding.resource),
            frame: None,
        }));
        Ok(Self { state, custody, owner: context.owner_id(), scope: context.scope_id() })
    }
}
