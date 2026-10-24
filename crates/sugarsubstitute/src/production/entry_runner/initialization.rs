// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start headless coordination and its owned asynchronous entry preparation.
use super::*;
impl<C: EntryActivation> EntryRunner<C> {
    /// Start the sole execution owner and its cleanup scope without configuration or native admission.
    pub(super) fn initialize_execution(
        &mut self,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<(), Box<dyn Error>> {
        if self.execution.is_some() || self.closing {
            return Err("Production execution cannot start twice or after close".into());
        }
        self.execution = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.execution.as_ref().ok_or("Production execution missing")?.client();
        self.client = Some(client.clone());
        self.cleanup = Some(
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?,
        );
        Ok(())
    }
    /// Begin the original setup budget and actual entry pipeline without waiting for a presentation round trip.
    pub(in crate::production) fn begin_entry(&mut self) -> Result<(), Box<dyn Error>> {
        if self.original.is_some() || self.closing || self.execution.is_none() {
            return Err("Production entry cannot start twice or after close".into());
        }
        let client = self.client()?.clone();
        let anchor = Instant::now();
        let original = EntrySetupBudget::new(client.now()?)?;
        self.original = Some(original);
        self.setup_deadline = Some(
            anchor
                .checked_add(EntrySetupBudget::DURATION)
                .ok_or("Production setup deadline overflow")?,
        );
        self.source = Some(
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?,
        );
        self.finite = Some(
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?,
        );
        self.native = Some(client.create_native_scope(
            ScopeSpec { id: self.scopes.allocate()?, deadline: None },
            None,
        )?);
        let command = self.command.take().ok_or("Production command consumed")?;
        self.bootstrap = Some(EntryBootstrap::new(
            command.options,
            command.installation,
            command.diagnostics,
            self.executable.clone(),
            original,
        )?);
        self.authority = Some(CoordinatorAuthority::new(original, command.launch_selection)?);
        let source = self.source.take().ok_or("Original source scope missing")?;
        self.bootstrap
            .as_mut()
            .ok_or("Production bootstrap missing")?
            .start(&client, source, &self.jobs)?;
        Ok(())
    }
}
