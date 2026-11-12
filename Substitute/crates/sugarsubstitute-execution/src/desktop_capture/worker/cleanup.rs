// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt accepted returns before each later gate and retain uncertainty past the original cutoff.
use super::*;
use crate::capture_attach::control::CaptureCleanupSnapshot;
use crate::desktop_capture::{
    process_lifetime::ProcessLifetimeCapture, retirement::AcceptedCleanupCall,
};
impl DesktopCaptureActive {
    /// Preserve the historical original terminal; bounded uncertainty is a separate report.
    pub(super) fn cleanup(&mut self, core: &Core, now: Moment) {
        if self.completed {
            return;
        }
        self.publish_failure(now);
        if self.fields().is_none() {
            self.return_unissued(core);
            return;
        }
        let total_due = lock(&core.state)
            .jobs
            .get(&self.context.job_id())
            .and_then(|record| record.native_deadlines.as_ref())
            .is_some_and(|slots| slots.capture_cleanup_due(now)[1]);
        if total_due {
            self.report_uncertain(core);
            // Every accepted synchronous call has already adopted before this point.
            // No fresh provider/OS/proof issuance may follow the original total cutoff.
            return;
        }
        if !matches!(self.work.shared.cleanup.snapshot(), CaptureCleanupSnapshot::Latched { .. }) {
            return;
        }
        let Some(fields) = self.fields() else {
            return;
        };
        if self.proof.is_none() {
            match core.desktop_capture_bundle_retirement(&fields.original, &self.work.shared) {
                Ok(proof) => self.proof = Some(proof),
                Err(cause) => {
                    self.failed(core, error("capture_cleanup_authority", cause));
                    return;
                }
            }
        }
        if let Err(cause) = self.close_temporary(core) {
            self.failed(core, cause);
            return;
        }
        let registered = lock(&self.work.shared.state).registered_published;
        if registered {
            self.transfer_permanent(core);
        } else {
            self.close_unregistered(core);
        }
    }
    /// Return the pristine borrowed input outside every fence; Core then validates
    /// the actual untouched worker/record and detaches charged controls unlocked.
    fn return_unissued(&mut self, core: &Core) {
        if !self.input_returned {
            let Some(input) = self.work.inherited.as_mut() else {
                return;
            };
            if let Err(cause) = input.lease_mut().return_unissued() {
                self.failed(core, cause);
                return;
            }
            // Successful loan return precedes original input/control destruction.
            self.input_returned = true;
            drop(self.work.inherited.take());
        }
        let returned = super::super::unissued::UnissuedCaptureReturn::returned(self);
        match core.retire_unissued_desktop_capture(&returned) {
            Ok(()) => self.completed = true,
            Err(cause) => self.failed(core, error("capture_unissued_return", cause)),
        }
    }
    /// C++ retirement is only its own temporary IO fact, never Event/source retirement.
    fn close_temporary(&mut self, core: &Core) -> Result<(), NativeObservationError> {
        let proof = self.proof.as_ref().ok_or_else(|| invariant("capture_cleanup_proof"))?;
        let fields = self
            .work
            .capsule
            .as_mut()
            .and_then(|capsule| capsule.storage.fields.as_mut())
            .ok_or_else(|| invariant("capture_cleanup_fields"))?;
        if fields.outputs.attach != 0 {
            let retired = super::super::native_reply::decode(&fields.outputs.outer)
                .map_err(|cause| error("capture_cleanup_reply", cause))?
                .retired;
            if !retired {
                let mut call = AcceptedCleanupCall::accept(
                    core,
                    &self.work.shared,
                    &self.context,
                    &fields.original,
                    proof,
                    fields.driver.as_ref(),
                    &mut fields.outputs,
                )?;
                // Drop records this exact call's adoption even if it returns an error.
                call.run()?;
            }
            let facts = super::super::native_reply::decode(&fields.outputs.outer)
                .map_err(|cause| error("capture_cleanup_returned", cause))?;
            if !facts.retired || facts.pending || fields.outputs.foreign_attach {
                return Err(invariant("capture_cleanup_io_held"));
            }
        }
        if let Some(event) = &mut self.event {
            event.close(proof)?;
        }
        if let Some(input) = &mut self.work.inherited {
            if !input.lease_mut().retire(proof)? {
                return Err(invariant("capture_cleanup_input_held"));
            }
            // Actual input owner has returned checked closure, before its outer Box drops.
            drop(self.work.inherited.take());
        }
        Ok(())
    }
    /// Core alone associates the capsule with this exact Record and extracts guards.
    /// Its passive transfer performs no fresh native effect or proof/clock check.
    fn transfer_permanent(&mut self, core: &Core) {
        let Some(capsule) = self.work.capsule.as_ref() else {
            return;
        };
        let Some(event) = self.event.as_ref() else {
            return;
        };
        match core.transfer_desktop_capture(capsule, event) {
            Ok(()) => {}
            Err(cause) => {
                self.failed(core, error("capture_permanent_transfer", cause));
                return;
            }
        }
        let Some(capsule) = self.work.capsule.take() else {
            return;
        };
        let retained = ProcessLifetimeCapture::transferred(capsule);
        match self.work.delivery.publish(Ok(retained)) {
            Ok(()) => {}
            Err(original) => drop(original), // Its actual permanent capsule retains on Drop.
        }
        self.retirement_reported = true;
        self.completed = true;
        core.mailbox.deliver_wake();
    }
    /// Fully closed failed preparation has no permanent native output to transfer.
    fn close_unregistered(&mut self, core: &Core) {
        let Some(proof) = self.proof.as_ref() else {
            return;
        };
        let Some(fields) =
            self.work.capsule.as_mut().and_then(|capsule| capsule.storage.fields.as_mut())
        else {
            return;
        };
        match fields.driver.retire_prepared(proof) {
            Ok(true) => {}
            Ok(false) => return,
            Err(cause) => {
                self.failed(core, cause);
                return;
            }
        }
        {
            let state = lock(&core.state);
            let mut cleanup = lock(&self.work.shared.cleanup.state);
            let mut child = lock(&self.work.shared.state);
            cleanup.native_retired = true;
            cleanup.source_retired = true;
            cleanup.active = false;
            child.phase = DesktopCapturePhase::Retired;
            drop(child);
            drop(cleanup);
            drop(state);
        }
        self.work.shared.cleanup.release_memory();
        core.retire_native_custody(self.context.job_id());
        self.completed = true;
    }
    /// Exactly one report states retained uncertainty; late facts never rewrite it.
    fn report_uncertain(&mut self, core: &Core) {
        if self.retirement_reported {
            return;
        }
        self.retirement_reported = true;
        match self.work.delivery.publish(Err(CaptureAttachFailure::Uncertain)) {
            Ok(()) => {}
            Err(original) => drop(original),
        }
        core.mailbox.deliver_wake();
    }
}
