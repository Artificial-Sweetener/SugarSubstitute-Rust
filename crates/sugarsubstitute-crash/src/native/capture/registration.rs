// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select and retain the genuine installed bundle before actual parent registration.
use super::*;
/// Run finite trusted selection/loading/kernel work on the same retained worker loan.
/// Every output is adopted before status; no owner mutex crosses a provider/native call.
pub(super) fn register(
    fields: &mut Fields,
    input: CaptureStartInput,
    owner: CoordinatorKey,
    context: &JobContext,
) -> Result<(), CrashError> {
    fields.input = Some(input);
    let admission =
        fields.admission.take().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let request = fields
        .input
        .as_mut()
        .and_then(|input| input.selection.take())
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let bundle_owner = match sugarsubstitute_supervision::installed_release::CaptureBundleOwner::new(
        request, admission,
    ) {
        Ok(owner) => owner,
        Err(failure) => {
            fields.original = Some(failure.original_origin().clone());
            return Err(CrashError::new(CaptureOperation::Runtime, failure));
        }
    };
    fields.original = Some(bundle_owner.original_origin().clone());
    fields.selected = Some(bundle_owner);
    fields
        .selected
        .as_mut()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?
        .select()
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let bundle_owner =
        fields.selected.take().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let lease = match bundle_owner.take_verified() {
        Ok(lease) => lease,
        Err(owner) => {
            fields.selected = Some(owner);
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
    };
    fields.lease = Some(lease);
    let lease =
        fields.lease.take().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let parts = match lease.into_parts() {
        Ok(parts) => parts,
        Err(lease) => {
            fields.lease = Some(lease);
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
    };
    // Complete boxed module parts are counted within the original callback claim.
    const _: () = assert!(
        size_of::<sugarsubstitute_supervision::installed_release::CaptureBundleParts>() + 128
            < crate::native::CALLBACK_PIN_BYTES
    );
    fields.parts = Some(Box::new(parts));
    let parts =
        fields.parts.as_ref().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let facts = parts
        .retention
        .registration_facts()
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let build = facts.build().map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    if build.is_empty() || build.len() > 64 {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    fields.build[..build.len()].copy_from_slice(build.as_bytes());
    fields.build_length = build.len();
    let handler = pins::wide(
        parts
            .retention
            .handler_path()
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?,
    )?;
    let input =
        fields.input.as_ref().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    fields
        .storage
        .as_mut()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?
        .resolve(&input.capture_root, context)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let capture_origin = if let Some(source) = &input.recovery {
        if source.key().run().owner != owner {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        fields
            .storage
            .as_mut()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?
            .create_recovery_capture_generation(source, context)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(
            sugarsubstitute_contracts::crash::RecoveryCaptureIdentity::from_grant(source.key()),
        )
    } else {
        fields
            .storage
            .as_mut()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?
            .create_capture_generation(owner, context)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Coordinator(owner)
    };
    let capture_key = capture_origin
        .capture_key()
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let generation = fields
        .storage
        .as_ref()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?
        .generation()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let database = pins::wide(generation.database_path())?;
    let metrics = pins::wide(generation.metrics_path())?;
    pins::deadline(context)?;
    let claim =
        fields.pin_claim.take().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let parts =
        fields.parts.take().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
    let api = match bridge::load(parts, claim) {
        Ok(api) => api,
        Err((cause, claim, parts)) => {
            fields.pin_claim = claim;
            fields.parts = parts;
            return Err(cause);
        }
    };
    fields.api = Some(api);
    let mut coordinator = [0u8; 96];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in
        capture_key.namespace.bytes().iter().chain(capture_key.epoch.bytes()).enumerate()
    {
        coordinator[index * 2] = HEX[(byte >> 4) as usize];
        coordinator[index * 2 + 1] = HEX[(byte & 15) as usize];
    }
    let deadline =
        context.deadline().ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?;
    let encoded = fields
        .execution
        .encode_counter_deadline(deadline)
        .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
    let deadline_counter = encoded.ticks();
    let frequency = encoded.frequency();
    pins::deadline(context)?;
    fields
        .original
        .as_ref()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?
        .check_effect()
        .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
    // SAFETY: the current-process pseudo-handle is borrowed only for the checked finite metadata call.
    let creator = identity(unsafe { GetCurrentProcess() })?;
    fields.creator = Some(
        crate::capture::CaptureCreator::new(creator.process_id, creator.creation_time_100ns)
            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?,
    );
    pins::deadline(context)?;
    let slots = fields.pins.reserve_outputs()?;
    let output = api.start(&bridge::Input {
        handler: handler.as_ptr(),
        handler_length: handler.len() - 1,
        database: database.as_ptr(),
        database_length: database.len() - 1,
        metrics: metrics.as_ptr(),
        metrics_length: metrics.len() - 1,
        build: fields.build.as_ptr(),
        build_length: fields.build_length,
        coordinator: coordinator.as_ptr(),
        coordinator_length: coordinator.len(),
        deadline_counter,
        counter_frequency: frequency,
    });
    // This precedes every ABI status/version/error check, including failure.
    let adoption = fields.pins.retain_outputs(slots, [output.process, output.thread]);
    if fields.pins.values[slots[0]].is_some() {
        fields.process = Some(slots[0]);
    }
    fields.pins.release_unused_outputs();
    adoption?;
    let identity_error = if let Some(index) = fields.process {
        match identity(fields.pins.handle(index)?) {
            Ok(value) => {
                fields.handler = Some(value);
                None
            }
            Err(cause) => Some(cause),
        }
    } else {
        None
    };
    if let Some(cause) = &identity_error {
        fields.retain(cause.clone());
    }
    fields.cleanup_codes(&output)?;
    if output.version != 1 || output.registered > 1 || output.pending_io > 1 {
        return Err(CrashError::invalid(CaptureOperation::Registration));
    }
    fields.registered = output.registered == 1;
    if output.error != 0 {
        let mut cause = native_code(output.error, CaptureOperation::Registration);
        if let Some(secondary) = identity_error {
            cause = cause.with_secondary(secondary);
        }
        if output.cleanup_error != 0 {
            cause = cause
                .with_secondary(native_code(output.cleanup_error, CaptureOperation::Retirement));
        }
        return Err(cause);
    }
    if let Some(cause) = identity_error {
        return Err(cause);
    }
    if output.cleanup_error != 0 {
        return Err(native_code(output.cleanup_error, CaptureOperation::Retirement));
    }
    pins::deadline(context)?;
    if !fields.registered || fields.handler.is_none() {
        return Err(CrashError::invalid(CaptureOperation::Registration));
    }
    Ok(())
}
