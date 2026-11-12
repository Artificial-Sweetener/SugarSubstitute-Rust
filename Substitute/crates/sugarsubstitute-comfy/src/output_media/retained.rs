// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy a caller-owned verified session original into independent disposable native source custody.

use super::{
    ComfyOutputFormat, OutputArtifactIdentity, OutputTransferError, encoded_copy::copy_media,
};
use std::io::{self, Read};
use sugarsubstitute_contracts::execution::{JobClass, JobContext, Operation};
use sugarsubstitute_supervision::cache::{CacheReader, Namespace, RuntimeCache, storage_key};

/// Admitted scratch for one bounded retained-original copy, excluding the caller's existing source pin.
pub const RETAINED_ORIGINAL_BYTES: u64 = 512 * 1024;

/// Stream one original from the caller's current reader position into a fresh independently pinned cache file.
/// The caller supplies existing authoritative session-reader custody and seeks to the original start first.
/// This operation grants no backend, filesystem-path or process authority and never closes the input loan.
/// Its returned reader and final release belong to the storage worker; the session original stays authoritative.
/// Use a finite InfrastructureOrdinary/Infrastructure job with at least RETAINED_ORIGINAL_BYTES scratch admission.
/// # Errors
/// Preserves context/admission, original read, exact length/digest/format, cache publication and pin failures.
/// Rejected partial copies remain unpublished under the existing cache owner's recovery lifecycle.
pub fn retain_original(
    reader: &mut impl Read,
    format: ComfyOutputFormat,
    expected: OutputArtifactIdentity,
    cache: RuntimeCache,
    now: u64,
    context: &JobContext,
) -> Result<CacheReader<'static>, OutputTransferError> {
    if context.spec().class != JobClass::InfrastructureOrdinary
        || context.spec().operation != Operation::Infrastructure
        || expected.bytes() < format.minimum_bytes()
    {
        return Err(OutputTransferError::invalid("retained_original_context"));
    }
    check(context).map_err(|cause| OutputTransferError::new("retained_original_context", cause))?;
    let _claim = context
        .memory()
        .claim(RETAINED_ORIGINAL_BYTES)
        .map_err(|cause| OutputTransferError::new("retained_original_admission", cause))?;
    let identity = [
        b"session-retained-v1".as_slice(),
        expected.digest().as_slice(),
        &expected.bytes().to_le_bytes(),
        format.content_type().as_bytes(),
    ]
    .concat();
    let key = storage_key(&identity);
    let mut entry = cache
        .fresh(Namespace::ComfyOutputs, &key, expected.bytes(), now, context)
        .map_err(|cause| OutputTransferError::new("retained_original_cache", cause))?;
    entry
        .write_with(format.entry_name(), expected.bytes(), context, |output| {
            copy_media(format, expected.bytes(), Some(expected), output, |bytes| {
                check(context)?;
                reader.read(bytes)
            })
        })
        .map_err(|cause| OutputTransferError::new("retained_original_body", cause))?;
    check(context).map_err(|cause| OutputTransferError::new("retained_original_context", cause))?;
    entry
        .publish(context)
        .map_err(|cause| OutputTransferError::new("retained_original_publish", cause))?;
    entry
        .into_file_reader(format.entry_name(), context)
        .map_err(|cause| OutputTransferError::new("retained_original_pin", cause))
}

/// Check the owning execution boundary between every finite unit of original I/O.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
