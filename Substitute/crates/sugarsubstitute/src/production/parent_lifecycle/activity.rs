// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence new work on failure while draining independent original retirement facts.
use super::*;

impl ParentLifecycle {
    /// Consume one immutable global batch without allowing one failed cleanup
    /// admission to starve already admitted profile or pipe retirement.
    pub(crate) fn activity(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        self.activity_failure_origin = ParentActivityFailureOrigin::Coordinator;
        let mut failed = None;
        self.activity_phase(resources, &mut failed, "latch_terminal", |owner, resources| {
            owner.latch_terminal(batch, resources)
        });
        self.activity_phase(resources, &mut failed, "frontend_refresh", |owner, resources| {
            owner.progress_frontend_refresh(batch, resources)
        });
        self.activity_phase(resources, &mut failed, "consume_profile", |owner, resources| {
            owner.consume_profile(batch, resources)
        });
        self.activity_phase(
            resources,
            &mut failed,
            "finish_main_retirement",
            |owner, resources| owner.finish_main_retirement(resources),
        );
        if !self.closed {
            self.activity_phase(resources, &mut failed, "active_admission", |owner, resources| {
                owner.expire_acquisition(resources.execution.client.now()?)?;
                owner.expire_recovery(resources.execution.client.now()?)?;
                owner.advance_backend_commands(resources)
            });
        }
        while let Some(observation) = resources.session.control.take_observation() {
            self.activity_phase(resources, &mut failed, "observe_pipe", |owner, resources| {
                owner.observe(observation, resources)
            });
        }
        // Final pipe observations precede terminal acceptance, including the real
        // MainRetired/write facts queued immediately before native retirement.
        self.activity_phase(resources, &mut failed, "consume_terminal", |owner, resources| {
            owner.consume_terminal(resources)
        });
        if self.closed {
            self.activity_phase(resources, &mut failed, "finish_disconnect", |owner, resources| {
                owner.finish_disconnect(resources)
            });
        } else {
            self.activity_phase(resources, &mut failed, "active_delivery", |owner, resources| {
                owner.check_main_deadline(resources)?;
                owner.advance_attempt_cleanup(resources)?;
                owner.advance_workflow_watch(resources);
                owner.resume_acquisition_receive(resources)?;
                owner.send_next(resources)?;
                owner.receive_next(resources)
            });
        }
        match failed {
            None => Ok(()),
            Some(ParentActivityFailureOrigin::DesktopPipeTerminal) => {
                Err(ParentError::DesktopStopped)
            }
            Some(ParentActivityFailureOrigin::Coordinator) => Err(ParentError::OwnerFailed),
        }
    }

    fn activity_phase(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
        failed: &mut Option<ParentActivityFailureOrigin>,
        step: &'static str,
        operation: impl FnOnce(&mut Self, &mut ParentResources<'_, '_>) -> Result<()>,
    ) {
        let result = operation(self, resources);
        if let Some(origin) =
            self.accept_activity_result(result, step, resources.profile.snapshot().pending)
        {
            failed.get_or_insert(origin);
            resources.session.control.close();
        }
    }

    /// Retain exact causes in observation order and fence all subsequent admissions.
    /// Independent retirement phases remain eligible after this transition.
    pub(super) fn accept_activity_result(
        &mut self,
        result: Result<()>,
        step: &'static str,
        native_profile_pending: bool,
    ) -> Option<ParentActivityFailureOrigin> {
        let Err(cause) = result else {
            return None;
        };
        tracing::error!(operation="parent_lifecycle_transition_failed",step,run=?self.run,
            main_stage=?self.main.stage(),main_identity=?self.main.snapshot().identity,
            profile_pending=self.profile_pending.is_some(),native_profile_pending,
            attempt_pending=self.attempt_cleanup.is_some(),read=self.read,write=self.write.is_some(),
            rejection=?cause.downcast_ref::<ParentError>(),
            "Actual parent transition rejected; original cause retained");
        let origin = self.activity_failure_origin;
        self.retain_failure(cause);
        self.closed = true;
        Some(origin)
    }
}
