// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle actual fixed-record custody through separately latched finite cleanup.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
/// Preserve observed target or prove this exact retained stage's actual activation.
pub(crate) enum PublicationRetirement<const N: usize> {
    /// Observe the existing target without claiming this stage was activated.
    Preserved(RecordRead<N>),
    /// Prove original staged inode and bytes were actually activated and closed.
    Activated([u8; N]),
}
/// Settle without rewriting, reopening or renewing normal publication authority.
/// # Errors
/// Preserve actual custody on changed generation, foreign or renewed cleanup,
/// elapsed cleanup, clock failure, or native verification/read/flush/close failure.
pub(crate) fn retire_publication<const N: usize>(
    file: &HeldFile<N>,
    expected: Option<&[u8; N]>,
    context: &JobContext,
) -> Result<PublicationRetirement<N>, ClaimError> {
    retire_with_boundary(file, expected, context, &mut native::NativeRecordBoundary)
}
pub(crate) fn retire_with_boundary<const N: usize>(
    file: &HeldFile<N>,
    expected: Option<&[u8; N]>,
    context: &JobContext,
    boundary: &mut dyn RecordBoundary<N>,
) -> Result<PublicationRetirement<N>, ClaimError> {
    if !file.handle.budget().is_ordinary() {
        return Err(file.handle.budget().rejection("fixed_record_ordinary_origin"));
    }
    if matches!(file.handle.budget(), crate::native_claims::files::Budget::Custody(_)) {
        file.handle.budget().check()?;
    } else {
        context::live(context)?;
    }
    if !file.is_locked() || !(16..=1024).contains(&N) {
        return Err(ClaimError::invalid("retirement_without_exclusion"));
    }
    let operation = crate::protected_storage::exclusion::operation::Operation::begin(file)?;
    let mut pending =
        crate::protected_storage::exclusion::operation::Loan::take(&file.publication, &operation);
    if pending.is_none() {
        *pending = Some(PublicationBinding::cleanup(expected, context, &file.handle.budget())?);
    }
    let binding =
        pending.as_mut().ok_or_else(|| ClaimError::invalid("missing_publication_binding"))?;
    macro_rules! step {
        ($effect:expr) => {{
            binding.context.validate_cleanup(context)?;
            let result = $effect;
            ClaimError::finish(result, binding.context.validate_cleanup(context))?
        }};
    }
    binding.context.validate_cleanup(context)?;
    let mut retained =
        crate::protected_storage::exclusion::operation::Loan::take(&file.stage, &operation);
    if let Some(stage) = retained.as_ref()
        && stage.activated.is_some()
        && stage.activated.as_ref() != expected
    {
        return Err(ClaimError::invalid("changed_retirement_generation"));
    }
    step!(boundary.close_temporary(
        file,
        &mut || binding.context.validate_cleanup(context),
        &operation
    ));
    step!(boundary.verify_exclusion(file));
    let outcome = match retained.as_mut() {
        None => PublicationRetirement::Preserved(step!(boundary.read_shape(
            file,
            None,
            &mut || binding.context.validate_cleanup(context),
            &operation
        ))),
        Some(stage) if stage.activated.is_some() => {
            let actual =
                stage.activated.ok_or_else(|| ClaimError::invalid("missing_stage_activation"))?;
            publication::settle_activation(file, stage, boundary, &operation, || {
                binding.context.validate_cleanup(context)
            })?;
            PublicationRetirement::Activated(actual)
        }
        Some(stage) => {
            if boundary.stage_is_open(stage) {
                step!(boundary.verify_stage(file, stage));
            }
            step!(boundary.close_stage(stage));
            PublicationRetirement::Preserved(step!(boundary.read_shape(
                file,
                None,
                &mut || binding.context.validate_cleanup(context),
                &operation
            )))
        }
    };
    *retained = None;
    *pending = None;
    Ok(outcome)
}
