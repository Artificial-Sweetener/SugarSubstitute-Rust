// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Count external driver calls without fabricating authenticated inputs.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_execution::{
    CaptureBundleAdmission, CaptureBundleRetirement, NativeCaptureDriver, NativeCaptureInput,
    NativeCaptureProgress, NativeCaptureReply, NativeCaptureStep, windows::NativeObservationError,
};
pub(super) struct InertDriver(pub(super) Arc<AtomicUsize>);
impl NativeCaptureDriver for InertDriver {
    fn prepare(&mut self, _: CaptureBundleAdmission) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn attach(
        &self,
        _: &NativeCaptureInput,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn poll(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn issue_step(
        &self,
        _: &NativeCaptureStep,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire_prepared(
        &mut self,
        _: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(false)
    }
    fn retained_pin_bytes(&self) -> u64 {
        0
    }
}
