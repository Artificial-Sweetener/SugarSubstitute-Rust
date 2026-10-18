// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit entry work only after the native entry presentation acknowledges its full first frame.
use super::*;
use crate::preparation::PreparationDriver;
use sugarsubstitute_contracts::presentation::PreparedFrame;
impl<C: EntryActivation> EntryRunner<C> {
    /// Inject only the clock boundary before any actual execution is constructed.
    #[cfg(test)]
    pub(in crate::production) fn set_test_clock(
        &mut self,
        clock: Arc<dyn sugarsubstitute_contracts::execution::Clock>,
    ) -> Result<(), Box<dyn Error>> {
        if self.execution.is_some() {
            return Err("Execution clock already bound".into());
        }
        self.clock = clock;
        Ok(())
    }
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
    /// Fund only the Root's complete initial splash through its shared execution and job owners.
    pub(in crate::production) fn start_presentation(
        &mut self,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<PreparationDriver, Box<dyn Error>> {
        self.initialize_execution(wake)?;
        let client = self.client()?.clone();
        let render =
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?;
        PreparationDriver::new(
            "en",
            client,
            render,
            self.jobs.clone(),
            sugarsubstitute_contracts::presentation::ConsolePresentation::Native,
            crate::preparation::FirstFrameContent::Complete,
        )
    }
    /// Begin the original setup budget and actual entry pipeline after native first-frame evidence.
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
                .checked_add(Duration::from_secs(30))
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
        self.authority = Some(CoordinatorAuthority::new(original)?);
        let source = self.source.take().ok_or("Original source scope missing")?;
        self.bootstrap
            .as_mut()
            .ok_or("Production bootstrap missing")?
            .start(&client, source, &self.jobs)?;
        Ok(())
    }
    /// Deliver one immutable bounded completion batch to render and entry without draining twice.
    pub(in crate::production) fn consume_presentation(
        &mut self,
        render: &mut PreparationDriver,
        max: NonZeroUsize,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let batch =
            self.client()?.drain_completions(max.min(NonZeroUsize::MIN.saturating_add(31)))?;
        let frame = render.consume(&batch).and_then(|frame| {
            if render.poll_retirement()? {
                render.finish_retirement()?;
            }
            Ok(frame)
        });
        let entry = self.consume_batch(&batch);
        match (frame, entry) {
            (Err(cause), _) | (_, Err(cause)) => Err(cause),
            (Ok(frame), Ok(_)) => Ok(frame),
        }
    }
    /// Borrow only actual child-owned presentation facts.
    pub(in crate::production) fn entry_splash(&self) -> EntrySplashState {
        self.child.entry_splash()
    }
    /// Complete the exact authenticated covered-splash handoff after actual Root hiding.
    pub(in crate::production) fn complete_entry_splash_handoff(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        self.child.complete_entry_splash_handoff(identity)
    }
    /// Expose deliberate lifetime closure to the native presentation pump.
    pub(in crate::production) const fn closing(&self) -> bool {
        self.closing
    }
    /// Observe whether the original entry setup budget has begun; this grants no admission proof.
    pub(in crate::production) const fn entry_setup_started(&self) -> bool {
        self.original.is_some()
    }

    /// Borrow real entry admission for presentation-lifetime pressure regressions.
    #[cfg(test)]
    pub(in crate::production) fn presentation_test_admission(
        &self,
    ) -> Result<(ExecutionClient, Scope, JobSequence), Box<dyn Error>> {
        Ok((
            self.client()?.clone(),
            self.client()?.create_scope(
                ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                self.cleanup.as_ref(),
            )?,
            self.jobs.clone(),
        ))
    }
}
