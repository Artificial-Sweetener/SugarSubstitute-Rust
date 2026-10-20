// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the one real execution owner independently of child/native retirement.

use std::{error::Error, num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionBatch, CompletionWake, Moment, Scope, ScopeSpec,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};

/// Create only after actual native wake, retaining partial setup through final joins.
pub(super) struct StartupExecution {
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    started: bool,
    shutdown_until: Option<Moment>,
    shutting_down: bool,
}
impl StartupExecution {
    /// Retain the configured original clock without starting workers.
    pub(super) fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock, owner: None, started: false, shutdown_until: None, shutting_down: false }
    }
    /// Start at most once, even if construction or subsequent owner setup fails.
    /// # Errors
    /// Preserve actual construction failure; another wake cannot reset this lifetime.
    pub(super) fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.started {
            return Err(std::io::Error::other("Startup execution already started").into());
        }
        self.started = true;
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        Ok(())
    }
    /// Borrow the existing typed client; never create a replacement owner.
    /// # Errors
    /// Rejects calls before the sole start has produced its actual execution owner.
    pub(super) fn client(&self) -> Result<ExecutionClient, Box<dyn Error>> {
        Ok(self
            .owner
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Startup execution absent"))?
            .client())
    }
    /// Create role/render scopes from the same original owner and supplied identities.
    pub(super) fn scope(&self, spec: ScopeSpec) -> Result<Scope, Box<dyn Error>> {
        use sugarsubstitute_contracts::execution::ExecutionPort;
        Ok(self.client()?.create_scope(spec, None)?)
    }
    /// Observe the same actual execution clock before fixing the first shutdown budget.
    pub(super) fn now(&self) -> Result<Moment, Box<dyn Error>> {
        Ok(self.client()?.now()?)
    }
    /// Identify whether construction retained the actual owner, including partial start.
    pub(super) const fn has_owner(&self) -> bool {
        self.owner.is_some()
    }
    /// Drain exactly one bounded immutable batch for every active composing owner.
    pub(super) fn drain(&self, maximum: NonZeroUsize) -> Result<CompletionBatch, Box<dyn Error>> {
        use sugarsubstitute_contracts::execution::ExecutionPort;
        Ok(self.client()?.drain_completions(maximum.min(NonZeroUsize::MIN.saturating_add(31)))?)
    }
    /// Latch the child transaction's first total cutoff without renewing it.
    /// This closes no executor admission while genuine child cleanup remains active.
    pub(super) fn retain_shutdown_cutoff(&mut self, until: Moment) {
        self.shutdown_until =
            Some(self.shutdown_until.map_or(until, |original| original.min(until)));
    }
    /// Close/poll execution only after actual child and rendering settlement.
    /// # Errors
    /// Missing cutoff or real runtime failure retains the original owner unjoined.
    pub(super) fn poll_shutdown(&mut self, owners_settled: bool) -> Result<bool, Box<dyn Error>> {
        if !owners_settled {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else {
            return Ok(true);
        };
        if !self.shutting_down {
            let until = self
                .shutdown_until
                .ok_or_else(|| std::io::Error::other("Original child shutdown cutoff absent"))?;
            self.shutting_down = true;
            owner.request_shutdown(until);
        }
        Ok(owner.poll_shutdown()?.settled)
    }
    /// Join only after real runtime settlement and the desktop's native/wake disposal.
    /// # Errors
    /// Actual final-join errors retain the consumed owner for desktop custody handling.
    pub(super) fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(owner) = &mut self.owner {
            if !self.shutting_down || !owner.poll_shutdown()?.settled {
                return Err(std::io::Error::other("Startup execution has not settled").into());
            }
            owner.finish_shutdown()?;
        }
        self.owner = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "execution/tests.rs"]
mod tests;
