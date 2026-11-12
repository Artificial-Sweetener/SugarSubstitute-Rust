// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose sequential probe/preparation families with separate scopes; only probes are time-bounded.
use super::*;
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};

/// Process-local matching facts only; authenticated file custody supplies actual launch authority.
pub(super) fn key() -> Result<CoordinatorKey> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([0x55; 32]),
        epoch: CoordinatorEpoch::new([0x45; 16])?,
    })
}

impl Engine {
    /// Fresh scope identities separate the second helper from the first helper's cancelled scopes.
    pub(super) fn launch(&mut self) -> Result<()> {
        let first = self.next_scope;
        self.next_scope = first.checked_add(3).ok_or("Updater scope identities exhausted")?;
        let work = self.client.create_scope(
            ScopeSpec { id: ScopeId::new(first)?, deadline: None },
            Some(&self.work),
        )?;
        let cleanup = match self.client.create_scope(
            ScopeSpec { id: ScopeId::new(first + 1)?, deadline: None },
            Some(&self.cleanup),
        ) {
            Ok(scope) => scope,
            Err(cause) => {
                work.cancel();
                return Err(cause.into());
            }
        };
        let scope = parent::LaunchScope {
            work,
            cleanup,
            native: ScopeSpec { id: ScopeId::new(first + 2)?, deadline: None },
            key: key()?,
        };
        let mut state = self.state()?;
        let Phase::Input(input) = &mut state.phase else {
            return Err("Updater launch input absent".into());
        };
        let parent = Parent::from_input(self.client.clone(), scope, input)
            .map_err(|cause| cause as Box<dyn Error>)?;
        state.phase = Phase::Empty;
        drop(state);
        self.parent = Some(parent);
        Ok(())
    }

    /// Adopt returned trial custody before reporting failure, so shutdown can publish or withdraw it.
    pub(super) fn observe_process(
        &mut self,
        batch: &CompletionBatch,
        jobs: &JobSequence,
    ) -> Result<()> {
        let Some(parent) = &mut self.parent else {
            return Ok(());
        };
        parent.advance(batch, jobs).map_err(|cause| cause as Box<dyn Error>)?;
        // Lock before taking unique custody; a failed composition check must leave it with Parent.
        let mut state = observed_state(&self.state)?;
        if !matches!(state.phase, Phase::Empty) {
            return Err("Updater process overlaps storage custody".into());
        }
        let Some(done) = parent.take_completion() else {
            return Ok(());
        };
        self.parent = None;
        match done.probe {
            Some(parent::ProbeCustody::Unstarted(input)) => {
                state.phase = Phase::Input(Some(*input))
            }
            Some(parent::ProbeCustody::Settled(probe)) => state.phase = Phase::Settled(probe),
            None => self.succeeded = done.succeeded && self.closing.is_none(),
        }
        if let Some(cause) = done.failure {
            return Err(cause as Box<dyn Error>);
        }
        if !done.succeeded && self.closing.is_none() {
            return Err("Updater helper did not complete successfully".into());
        }
        Ok(())
    }
}
