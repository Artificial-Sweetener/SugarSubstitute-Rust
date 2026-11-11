// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Connect the trusted selected bundle to the exact borrowed native attachment ABI.
pub(super) mod loader;
use super::module_pin::{Fields, Loan, SLOT};
use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_execution::windows::NativeObservationError;
use sugarsubstitute_execution::{
    CaptureBundleAdmission, CaptureBundleRetirement, DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES,
    NativeCaptureDriver, NativeCaptureInput, NativeCaptureProgress, NativeCaptureReply,
    NativeCaptureStep,
};
use sugarsubstitute_supervision::installed_release::{CaptureBundleOwner, InstalledReleaseOwner};

/// A desktop process has one actual capture driver; construction performs no native effect.
#[derive(Debug)]
pub struct DesktopCaptureDriverAlreadyCreated;
impl fmt::Display for DesktopCaptureDriverAlreadyCreated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Desktop capture driver already created")
    }
}
impl Error for DesktopCaptureDriverAlreadyCreated {}
static CREATED: AtomicBool = AtomicBool::new(false);
struct Driver;
/// Construct one zero-sized adapter without allocating pre-admission storage.
/// Native owners and their original claims are installed only by actual prepare admission.
/// # Errors
/// Rejects another driver for this same process. Rejected job admission returns this
/// identical move-only adapter for use with the original authenticated input.
pub fn desktop_capture_driver()
-> Result<Box<dyn NativeCaptureDriver>, DesktopCaptureDriverAlreadyCreated> {
    CREATED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| DesktopCaptureDriverAlreadyCreated)?;
    Ok(Box::new(Driver))
}
const _: () = assert!(size_of::<Driver>() == 0);

fn failure(
    stage: &'static str,
    cause: impl Error + Send + Sync + 'static,
) -> NativeObservationError {
    NativeObservationError::new(stage, cause)
}
pub(super) fn invalid(stage: &'static str) -> NativeObservationError {
    failure(stage, std::io::Error::other("Actual capture driver state unavailable"))
}
impl NativeCaptureDriver for Driver {
    fn prepare(&mut self, admission: CaptureBundleAdmission) -> Result<(), NativeObservationError> {
        let context =
            admission.original_context().map_err(|e| failure("capture_driver_context", e))?.clone();
        let native = context
            .memory()
            .claim(DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES)
            .map_err(|e| failure("capture_driver_native_claim", e))?;
        let callback = context
            .memory()
            .claim(65_536)
            .map_err(|e| failure("capture_driver_callback_claim", e))?;
        if Fields::storage_bytes() > DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES as usize {
            return Err(invalid("capture_driver_layout"));
        }
        let mut loan = Loan::install(&SLOT, admission, native, callback)?;
        let fields = loan.fields()?;
        // Selection authority remains the same original issuance; no new reservation exists.
        let admission =
            fields.admission.take().ok_or_else(|| invalid("capture_driver_admission"))?;
        let owner = match CaptureBundleOwner::new(
            InstalledReleaseOwner::new().capture_selection(),
            admission,
        ) {
            Ok(owner) => owner,
            Err(cause) => {
                fields.original = Some(cause.original_origin().clone());
                return Err(failure("capture_bundle_construct", cause));
            }
        };
        fields.original = Some(owner.original_origin().clone());
        fields.selected = Some(owner);
        fields
            .selected
            .as_mut()
            .ok_or_else(|| invalid("capture_bundle_owner"))?
            .select()
            .map_err(|e| failure("capture_bundle_select", e))?;
        let owner = fields.selected.take().ok_or_else(|| invalid("capture_bundle_verified"))?;
        match owner.take_verified() {
            Ok(lease) => fields.lease = Some(lease),
            Err(owner) => {
                fields.selected = Some(owner);
                return Err(invalid("capture_bundle_unverified"));
            }
        }
        let lease = fields.lease.take().ok_or_else(|| invalid("capture_bundle_lease"))?;
        match lease.into_parts() {
            Ok(parts) => fields.parts = Some(parts),
            Err(lease) => {
                fields.lease = Some(lease);
                return Err(invalid("capture_bundle_parts"));
            }
        }
        let build = fields
            .parts
            .as_ref()
            .ok_or_else(|| invalid("capture_bundle_build_owner"))?
            .retention
            .registration_facts()
            .map_err(|e| failure("capture_bundle_facts", e))?
            .build()
            .map_err(|e| failure("capture_bundle_build", e))?
            .as_bytes();
        if build.is_empty() || build.len() > 64 {
            return Err(invalid("capture_bundle_build_length"));
        }
        fields.build[..build.len()].copy_from_slice(build);
        fields.build_length = build.len();
        loader::load(fields)?;
        Ok(())
    }
    fn attach(
        &self,
        input: &NativeCaptureInput,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        let (api, build, length) = SLOT.binding().ok_or_else(|| invalid("capture_driver_api"))?;
        if !input.matches_build(&build[..length]) {
            return Err(invalid("capture_driver_build_mismatch"));
        }
        // SAFETY: actual authenticated same-build process-pinned DLL receives
        // only exact synchronous borrowed ABI buffers retained by Execution.
        unsafe { (api.attach)(input, output) };
        Ok(())
    }
    fn poll(
        &self,
        input: &NativeCaptureProgress,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        let api = SLOT.api().ok_or_else(|| invalid("capture_driver_api"))?;
        // SAFETY: actual pinned ABI borrows original live callbacks/output for this call.
        unsafe { (api.poll)(input, output) };
        Ok(())
    }
    fn issue_step(
        &self,
        input: &NativeCaptureStep,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        let api = SLOT.api().ok_or_else(|| invalid("capture_driver_api"))?;
        // SAFETY: Execution supplies the exact one-shot accepted stage synchronously.
        unsafe { (api.step)(input, output) };
        Ok(())
    }
    fn retire(
        &self,
        input: &NativeCaptureProgress,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        let api = SLOT.api().ok_or_else(|| invalid("capture_driver_api"))?;
        // SAFETY: original borrowed cleanup tuple and output remain live through return.
        unsafe { (api.retire)(input, output) };
        Ok(())
    }
    fn retire_prepared(
        &mut self,
        proof: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        let Some(mut loan) = Loan::take(&SLOT)? else { return Ok(true) };
        let fields = loan.fields()?;
        let original =
            fields.original.as_ref().ok_or_else(|| invalid("capture_driver_cleanup_origin"))?;
        if !proof.matches_origin(original) {
            return Err(invalid("capture_driver_cleanup_target"));
        }
        if fields.pinned != 0 || fields.pin_uncertain {
            return Ok(false);
        };
        loader::close_loaded(fields, proof)?;
        if let Some(owner) = fields.selected.take()
            && let Err(failed) = owner.retire(proof)
        {
            fields.selected = Some(failed.owner);
            return Err(failure("capture_selection_retire", failed.cause));
        }
        if fields.lease.is_some() {
            return Ok(false);
        };
        if let Some(parts) = fields.parts.take() {
            let sugarsubstitute_supervision::installed_release::CaptureBundleParts {
                retention,
                loaded,
                pinned,
            } = parts;
            match retention.retire(proof) {
                Ok(()) => {
                    drop(loaded);
                    drop(pinned)
                }
                Err(failed) => {
                    fields.parts =
                        Some(sugarsubstitute_supervision::installed_release::CaptureBundleParts {
                            retention: failed.retention,
                            loaded,
                            pinned,
                        });
                    return Err(failure("capture_bundle_retire", failed.cause));
                }
            }
        }
        loan.return_closed()?;
        Ok(true)
    }
    fn retained_pin_bytes(&self) -> u64 {
        SLOT.retained_bytes()
    }
}
