// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire source pins and the actual fresh handler only after caller capture policy permits it.
use super::*;
impl Capture {
    /// Fence all new attachment admission and cancel its actual existing owner.
    pub(in crate::production::recovery_supervisor) fn request_close(&mut self) {
        self.closing = true;
        self.pin = None;
        if let Some(attachment) = &self.attachment {
            attachment.cancel();
        }
    }
    /// Submit checked native retirement under the first original containment deadline.
    /// Busy retains the same owner; failed or uncertain native output custody stays observable.
    pub(in crate::production::recovery_supervisor) fn close(
        &mut self,
        client: &ExecutionClient,
        cleanup: &Scope,
        jobs: &JobSequence,
        budget: ShutdownBudget,
    ) -> Result<bool> {
        self.request_close();
        if self.unresolved_rejection {
            return Ok(false);
        }
        self.rejected_input = None;
        if let Some(watch) = &mut self.unbound_watch {
            let Some(done) = watch.try_take_completion() else {
                return Ok(false);
            };
            let valid = done.scope_id == watch.scope_id()
                && done.spec == watch.spec()
                && done.memory.bytes() == watch.spec().reservation_bytes
                && done.native_handles.is_none();
            self.unbound_terminal = Some(done);
            if !valid {
                return Err("Fresh rejected watch terminal differs".into());
            }
            super::super::channel::terminal::classify(
                &self
                    .unbound_terminal
                    .as_ref()
                    .ok_or("Fresh rejected watch terminal missing")?
                    .outcome,
                true,
            )?;
            self.unbound_watch = None;
        }
        if self.attachment_original.is_some() && self.attachment_terminal.is_none() {
            return Ok(false);
        }
        if !self.registration_admitted || self.registration.snapshot().state == CaptureState::Closed
        {
            return Ok(true);
        }
        if matches!(
            self.registration.snapshot().state,
            CaptureState::Registered | CaptureState::Degraded | CaptureState::Closing
        ) {
            let spec = JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(budget.settle_until),
                reservation_bytes: 4096,
                replacement: None,
            };
            match self.registration.submit_close(client, cleanup, spec) {
                Ok(()) | Err(sugarsubstitute_crash::CrashAdmissionError::Busy) => {}
                Err(cause) => return Err(Box::new(cause)),
            }
        }
        Ok(self.registration.snapshot().state == CaptureState::Closed)
    }
}
