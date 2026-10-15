// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate bounded bytes through one retained staging inode; codecs stay with owners.
mod boundary;
mod context;
pub(super) mod native;
pub(crate) mod publication;
pub(crate) mod retirement;
mod streaming;
#[cfg(test)]
pub(crate) mod tests;
use super::{exclusion::HeldFile, paths, security};
#[cfg(test)]
use crate::native_claims::lock;
use crate::{ClaimError, native_claims::files::Handle};
use boundary::RecordBoundary;
pub(crate) use native::random;
pub(crate) use publication::{PublicationAuthority, PublicationBinding, Stage, publish};
pub(crate) use retirement::{PublicationRetirement, retire_publication};
pub(crate) use streaming::StreamingPort;
pub(crate) use streaming::layout::binding as diagnostic_object_binding;
pub(crate) use streaming::layout::object_payload_group as diagnostic_payload_group_digest;
pub(crate) use streaming::layout::subject as diagnostic_subject_digest;
pub(crate) use streaming::leaf::Leaf as DiagnosticLeaf;
/// Distinguish protected absence, exact empty shape and complete bounded bytes.
pub(crate) enum RecordRead<const N: usize> {
    /// No source inode was opened at the protected target.
    Missing,
    /// The regular protected source was actually empty and checked closed.
    Empty,
    /// Exactly N bytes were read from the retained protected source and checked closed.
    Present([u8; N]),
}
/// Read at most1024 fixed bytes through retained protected source custody.
/// # Errors
/// Rejects invalid bounds, shape, security, identity or native read/close failure;
/// adopted failed-close outputs remain with the original exclusion owner.
pub(crate) fn read_shape<const N: usize>(
    file: &HeldFile<N>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<RecordRead<N>, ClaimError> {
    read_with_boundary(file, context, &mut native::NativeRecordBoundary)
}
/// Read through the same primitive boundary while retaining failed-close custody.
pub(crate) fn read_with_boundary<const N: usize>(
    file: &HeldFile<N>,
    context: &sugarsubstitute_contracts::execution::JobContext,
    boundary: &mut dyn RecordBoundary<N>,
) -> Result<RecordRead<N>, ClaimError> {
    if !file.handle.budget().is_ordinary() {
        return Err(file.handle.budget().rejection("fixed_record_ordinary_origin"));
    }
    let operation = super::exclusion::operation::Operation::begin(file)?;
    let mut pending = super::exclusion::operation::Loan::take(&file.publication, &operation);
    let new_observation = pending.is_none();
    if new_observation {
        *pending = Some(PublicationBinding::read(context, &file.handle.budget())?);
    }
    let binding =
        pending.as_mut().ok_or_else(|| ClaimError::invalid("missing_record_read_binding"))?;
    binding.context.validate(context)?;
    let result =
        boundary.read_shape(file, None, &mut || binding.context.validate(context), &operation);
    let result = ClaimError::finish(result, binding.context.validate(context));
    if new_observation && result.is_ok() {
        *pending = None;
    }
    result
}
/// Clear the observed activation only after actual cleanup succeeds.
/// # Errors
/// Preserve the original activation if the supplied checked cleanup fails.
pub(crate) fn finish_activation<const N: usize>(
    state: &mut Option<[u8; N]>,
    desired: &[u8; N],
    cleanup: impl FnOnce() -> Result<(), ClaimError>,
) -> Result<bool, ClaimError> {
    let Some(actual) = state.as_ref() else {
        return Ok(false);
    };
    let matched = actual == desired;
    cleanup()?;
    *state = None;
    Ok(matched)
}
/// Encode exact bytes into canonical lowercase hexadecimal naming components.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [DIGITS[(byte >> 4) as usize] as char, DIGITS[(byte & 15) as usize] as char]
        })
        .collect()
}
