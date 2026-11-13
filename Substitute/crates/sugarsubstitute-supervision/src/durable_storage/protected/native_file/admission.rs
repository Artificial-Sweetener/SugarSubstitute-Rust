// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume a complete caller input only after physical-cell admission succeeds.
use super::*;
impl NativeFile {
    /// Reserve the physical cell from the caller's complete original input slot.
    /// # Errors
    /// Validation leaves the same name, admission, credits and causes in the slot.
    /// Successful reservation consumes it before any native producer may run.
    pub(crate) fn reserve(
        id: u64,
        bound: u64,
        grant: FileGrant,
        input: &mut Option<(NativeName, NativeFileAdmission)>,
    ) -> Result<Self, ClaimError> {
        let missing = || {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        };
        let (name, admission) = input.as_mut().ok_or_else(missing)?;
        let invalid = id == 0
            || (bound == 0
                && !matches!(
                    grant,
                    FileGrant::MetadataReadOnly
                        | FileGrant::InventoryMetadata
                        | FileGrant::DirectoryScan
                ));
        let path_valid =
            name.path().is_ok() && name.capacity().is_ok_and(|bytes| bytes <= 192 * 1024);
        if invalid || !path_valid {
            return Err(match admission {
                NativeFileAdmission::Selected { .. } => ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::identity(),
                ),
                NativeFileAdmission::Diagnostic { grant, .. } => {
                    ClaimError::diagnostic_allocation(grant.reject_native_state())
                }
                #[cfg(test)]
                NativeFileAdmission::Ordinary { budget }
                | NativeFileAdmission::Symbol { budget, .. } => {
                    budget.rejection("diagnostic_file_capacity")
                }
            });
        }
        let selected_context = match admission {
            #[cfg(test)]
            NativeFileAdmission::Ordinary { budget }
            | NativeFileAdmission::Symbol { budget, .. } => {
                if !budget.is_ordinary() || !name.is_ordinary() {
                    let operation = if matches!(admission, NativeFileAdmission::Ordinary { .. }) {
                        "native_file_ordinary_origin"
                    } else {
                        "native_file_symbol_origin"
                    };
                    return Err(match admission {
                        NativeFileAdmission::Ordinary { budget }
                        | NativeFileAdmission::Symbol { budget, .. } => budget.rejection(operation),
                        _ => missing(),
                    });
                }
                None
            }
            NativeFileAdmission::Selected { original, allocation, output, causes } => {
                if !name.matches_selected(original)
                    || !allocation.original().matches(original)
                    || !output.original().matches(original)
                    || causes.matches_selected(original).is_err()
                {
                    return Err(ClaimError::capture_allocation(
                        crate::installed_release::CaptureCapacityFailure::identity(),
                    ));
                }
                Some(
                    original
                        .original()
                        .original_context()
                        .map_err(|cause| causes.selected_control(cause, false))?
                        .clone(),
                )
            }
            NativeFileAdmission::Diagnostic { grant, causes, .. } => {
                if !name.matches_original(grant) || causes.matches_original(grant).is_err() {
                    return Err(ClaimError::diagnostic_allocation(grant.reject_native_state()));
                }
                None
            }
        };
        let (name, admission) = input.take().ok_or_else(missing)?;
        let (handle, allocation, original, counted, causes) = match admission {
            #[cfg(test)]
            NativeFileAdmission::Ordinary { budget } => (
                Handle::new(HANDLE::default(), budget.clone()),
                NativeFileAllocation::Ordinary { _budget: budget },
                None,
                false,
                NativeCauseDestinations::ordinary(),
            ),
            #[cfg(test)]
            NativeFileAdmission::Symbol { budget, original, output } => (
                Handle::reserve_output(budget.clone(), output),
                NativeFileAllocation::Ordinary { _budget: budget },
                Some(original),
                true,
                NativeCauseDestinations::ordinary(),
            ),
            NativeFileAdmission::Selected { original, allocation, output, causes } => {
                let mut handle_input = Some((original.clone(), output));
                let handle = match Handle::reserve_capture(&mut handle_input) {
                    Ok(handle) => handle,
                    Err(cause) => {
                        if let Some((_, output)) = handle_input {
                            *input = Some((
                                name,
                                NativeFileAdmission::Selected {
                                    original,
                                    allocation,
                                    output,
                                    causes,
                                },
                            ));
                        }
                        return Err(cause);
                    }
                };
                (
                    handle,
                    NativeFileAllocation::Selected { _original: original, _allocation: allocation },
                    selected_context,
                    true,
                    causes,
                )
            }
            NativeFileAdmission::Diagnostic { original, grant, allocation, causes } => (
                Handle::new(HANDLE::default(), Budget::Diagnostic(grant.clone())),
                NativeFileAllocation::Diagnostic { original: grant, _allocation: allocation },
                Some(original),
                false,
                causes,
            ),
        };
        Ok(Self {
            fields: Some(Box::new(Fields {
                id,
                handle,
                path: name,
                facts: None,
                bound,
                stage: grant == FileGrant::WritableStage,
                deleted: false,
                grant,
                counted,
                attempted: false,
                original,
                causes,
                first_failure: None,
                secondary_failure: None,
            })),
            allocation: Some(allocation),
        })
    }
}
