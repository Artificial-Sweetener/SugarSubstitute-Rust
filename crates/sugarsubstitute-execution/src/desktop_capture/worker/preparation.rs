// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Install the original capsule before selection and use fresh fences before dispatch.
use super::*;
use crate::{
    capture_bundle::CaptureBundleAdmission,
    desktop_capture::{call, native_abi::NativeCaptureProgress},
};
impl DesktopCaptureActive {
    /// Actual Core issuance consumes the sole2MiB bundle claim from this4MiB job.
    pub(super) fn prepare(&mut self) -> Result<(), NativeObservationError> {
        let core = self.core.upgrade().ok_or_else(|| invariant("capture_prepare_core"))?;
        let admission = crate::ExecutionClient { core: core.clone() }
            .capture_bundle_admission(&self.context)
            .map_err(|cause| error("capture_bundle_admission", cause))?;
        let original = admission.origin.clone();
        let driver = self.work.driver.take().ok_or_else(|| invariant("capture_prepare_driver"))?;
        // The original16KiB metadata preflight includes this exact Box payload.
        // Its outside capsule/Active guard already exists before allocation.
        let fields = Box::new(ProcessCaptureFields {
            custody: self.work.shared.clone(),
            driver,
            original,
            context: self.context.clone(),
            _callback_signal: self.work.signal.clone(),
            outputs: CallOutputs::default(),
        });
        let Some(capsule) = &mut self.work.capsule else {
            self.work.driver = Some(fields.driver);
            return Err(invariant("capture_prepare_capsule"));
        };
        if let Err(fields) = capsule.storage.install(fields) {
            self.work.driver = Some(fields.driver);
            return Err(invariant("capture_prepare_duplicate"));
        }
        self.prepare_driver(admission)?;
        let fields = self.fields().ok_or_else(|| invariant("capture_event_original"))?;
        let claim = core
            .desktop_capture_event_claim(&fields.original, &self.work.shared)
            .map_err(|cause| error("capture_event_claim", cause))?;
        self.event = Some((self.work.event)(self.work.signal.clone(), claim)?);
        self.event.as_mut().ok_or_else(|| invariant("capture_event_owner"))?.prepare()?;
        let observed =
            core.clock.metadata().map_err(|cause| error("capture_prepare_clock", cause))?;
        let state = lock(&core.state);
        crate::capture_bundle::validate::effect(&core, &state, &self.context, observed, true)
            .map_err(|cause| error("capture_prepare_current", cause))?;
        let cleanup = lock(&self.work.shared.cleanup.state);
        let mut stage = lock(&self.work.shared.state);
        if cleanup.stopped || stage.phase != DesktopCapturePhase::Preparing {
            return Err(invariant("capture_prepare_stopped"));
        }
        stage.phase = DesktopCapturePhase::Attaching;
        self.prepared = true;
        Ok(())
    }
    /// Fields were installed BEFORE the external selector may panic/create outputs.
    fn prepare_driver(
        &mut self,
        admission: CaptureBundleAdmission,
    ) -> Result<(), NativeObservationError> {
        self.fields_mut()
            .ok_or_else(|| invariant("capture_prepare_fields"))?
            .driver
            .prepare(admission)
    }
    /// Borrow callbacks through one synchronous call; outputs are adopted before error.
    pub(super) fn advance(&mut self, core: &Core) -> Result<(), NativeObservationError> {
        let event =
            self.event.as_ref().ok_or_else(|| invariant("capture_dispatch_event"))?.value.raw();
        let input =
            self.work.inherited.as_ref().ok_or_else(|| invariant("capture_dispatch_input"))?;
        let custody = &self.work.shared;
        let context = &self.context;
        let fields = self
            .work
            .capsule
            .as_mut()
            .and_then(|capsule| capsule.storage.fields.as_mut())
            .ok_or_else(|| invariant("capture_dispatch_fields"))?;
        let attach = fields.outputs.attach;
        let driver = fields.driver.as_ref();
        if !self.attached {
            self.attached = true;
            call::invoke(
                core,
                custody,
                context,
                driver,
                &mut fields.outputs,
                |cleanup, issue, output| {
                    let native = input.lease().native_input(cleanup, issue, event)?;
                    driver.attach(&native, output)
                },
            )?;
        } else {
            call::invoke(
                core,
                custody,
                context,
                driver,
                &mut fields.outputs,
                |cleanup, issue, output| {
                    driver.poll(
                        &NativeCaptureProgress { version: 2, size: 48, attach, cleanup, issue },
                        output,
                    )
                },
            )?;
        }
        if let Some(cause) = fields.outputs.native_failures.failure() {
            return Err(error("capture_native_operation", cause));
        }
        let facts = super::super::native_reply::decode(&fields.outputs.outer)
            .map_err(|cause| error("capture_native_progress", cause))?;
        if facts.status == 5 && !facts.pending && !facts.registered && !facts.retired {
            // The exact ABI returns after one completed finite phase. Its next phase
            // has no I/O callback to wake us; signal the existing same owner once.
            self.work.signal.complete();
        }
        Ok(())
    }
}
