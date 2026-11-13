// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the canvas scope without taking ownership of a shared application executor.

use super::*;

/// Own the canvas's scope and optionally the standalone viewer's execution lifetime.
/// A shared application pumps completion metadata once for all of its active owners.
pub(crate) struct Runtime {
    owner: Option<ExecutionOwner>,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
}

impl Runtime {
    /// Create this canvas scope through the composing executor and retain no executor ownership.
    ///
    /// # Errors
    /// Preserve scope admission failures without acquiring or canceling a foreign scope.
    pub(crate) fn shared(client: ExecutionClient, spec: ScopeSpec) -> Result<Self, AdmissionError> {
        let scope = client.create_scope(spec, None)?;
        Ok(Self { owner: None, client, scope })
    }

    /// Retain the standalone viewer's actual executor alongside its single canvas scope.
    #[cfg(test)]
    pub(super) fn owned(
        owner: ExecutionOwner,
        client: ExecutionClient,
        spec: ScopeSpec,
    ) -> Result<Self, AdmissionError> {
        let mut runtime = Self::shared(client, spec)?;
        runtime.owner = Some(owner);
        Ok(runtime)
    }

    /// Transfer an already-retained standalone executor after scope admission succeeds.
    fn adopt_owner(mut self, owner: ExecutionOwner) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Drain only the standalone queue; shared completion delivery belongs to the application.
    pub(super) fn drain_standalone(&self) -> Result<(), Box<dyn Error>> {
        if self.owner.is_none() {
            return Ok(());
        }
        let batch =
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if batch.notifications.iter().any(|notice| notice.scope_id != self.scope.id()) {
            return Err("Canvas received a foreign execution scope notice".into());
        }
        Ok(())
    }

    /// Request retirement after the canvas's own jobs and disposal work have been observed.
    pub(super) fn request_shutdown(&mut self, deadline: Moment) {
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(deadline);
        } else {
            self.scope.cancel();
        }
    }

    /// Observe actual scope work, retained memory and native custody without stopping a shared executor.
    pub(super) fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if let Some(owner) = &mut self.owner {
            return Ok(owner.poll_shutdown()?.settled);
        }
        let scope = self.scope.snapshot();
        Ok(scope.cancellation_requested
            && scope.queued == 0
            && scope.running == 0
            && scope.terminal_unobserved == 0
            && scope.children_live == 0
            && scope.reserved_bytes == 0
            && scope.native_reservations == 0
            && scope.native_outputs_in_custody == 0)
    }

    /// Join only a standalone executor after its actual settlement.
    pub(super) fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.settled()? {
            return Err("Canvas execution has not settled".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        Ok(())
    }
}

impl Session {
    /// Retain the standalone owner before scope admission so failed startup can retire it.
    pub(super) fn start_owned(&mut self, spec: ScopeSpec) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() || self.closing {
            return Err("Canvas execution cannot be started".into());
        }
        let client = self.starting_owner.as_ref().ok_or("Canvas execution owner absent")?.client();
        let runtime = Runtime::shared(client, spec)?;
        let owner = self.starting_owner.take().ok_or("Canvas execution owner absent")?;
        self.start_runtime(runtime.adopt_owner(owner))
    }

    /// Start one retained canvas scope after its composing executor is ready.
    pub(crate) fn start_runtime(&mut self, runtime: Runtime) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() || self.starting_owner.is_some() || self.closing {
            return Err("Canvas execution cannot be restarted".into());
        }
        self.runtime = Some(runtime);
        self.initialization_pending = true;
        self.advance()
    }
}
