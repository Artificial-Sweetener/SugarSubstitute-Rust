// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mint genuine Root-local physical pins from the actual registered Root and grant.
use super::*;
use windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE},
    System::Threading::GetCurrentProcess,
};
impl ProtectedStorageRoot {
    pub(crate) fn admit_recorded_capture_read(
        &self,
        original: DiagnosticNativeGrant,
        origin: CaptureGenerationOrigin,
        context: &JobContext,
    ) -> Result<CaptureReadOwner, ClaimError> {
        let state =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_source_closed"))?;
        let generation = state
            .generation
            .as_ref()
            .filter(|generation| generation.ready)
            .ok_or_else(|| state.budget.rejection("capture_read_generation_unavailable"))?;
        if !matches!(generation.origin, CaptureGenerationOrigin::Coordinator(_))
            || generation.origin.coordinator().namespace != origin.coordinator().namespace
        {
            return Err(state.budget.rejection("recovery_read_root_source"));
        }
        let mut owner =
            self.admit_capture_read(original, origin.coordinator().namespace, context)?;
        let data =
            owner.data.as_mut().ok_or_else(|| state.budget.rejection("capture_read_closed"))?;
        data.origin = origin;
        data.coordinator = data
            .origin
            .capture_key()
            .map_err(|cause| ClaimError::new("recovery_read_key", cause))?;
        data.identities = [[0; 24]; 2];
        data.recovery_handles
            .try_reserve_exact(5)
            .map_err(|cause| ClaimError::new("recovery_read_slots", cause))?;
        if data.recovery_handles.capacity() != 5 {
            return Err(state.budget.rejection("recovery_read_slot_capacity"));
        }
        Ok(owner)
    }
    /// Existing-only relative opens share the exact SID/reparse/canonical close owner.
    /// Four chain pins plus one verification slot are the strict high-water; all
    /// seven distinct produced outputs are reserved even when verification slots reuse.
    pub(crate) fn prepare_recorded_capture_read_handles(
        &self,
        owner: &mut CaptureReadOwner,
        origin: CaptureGenerationOrigin,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        owner.current(context)?;
        let source =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_source_closed"))?;
        let (root, root_identity, root_slot) =
            source.root.as_ref().ok_or_else(|| source.budget.rejection("recovery_read_root"))?;
        let data =
            owner.data.as_mut().ok_or_else(|| source.budget.rejection("capture_read_closed"))?;
        if data.ready || !data.recovery_handles.is_empty() || data.origin != origin {
            return Err(source.budget.rejection("recovery_read_source_changed"));
        }
        let CaptureReadData {
            handles,
            identities,
            recovery_handles,
            origin,
            sid,
            sid_length,
            causes,
            original,
            pin,
            ..
        } = data.as_mut();
        let budget = Budget::Diagnostic(original.clone());
        resolution::with_phase(&budget, causes, context, resolution::ScratchRole::Path, |phase| {
            phase.capture_source(pin)?;
            let root_pin = source
                .handles
                .get(*root_slot)
                .ok_or_else(|| phase.invalid("recovery_read_root_pin"))?;
            paths::validate_object(root_pin, true, true, phase)?;
            if paths::file_identity(root_pin, phase)? != *root_identity {
                return Err(phase.invalid("recovery_read_root_changed"));
            }
            security::verify(root_pin, &sid[..*sid_length], phase)?;
            recovery_handles.push(Handle::new(HANDLE::default(), budget.clone()));
            phase.context(context)?;
            let mut raw = HANDLE::default();
            // SAFETY: actual registered Root pin stays retained by the original read fence.
            let duplicated = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    root_pin.raw(),
                    GetCurrentProcess(),
                    &mut raw,
                    0,
                    false,
                    DUPLICATE_SAME_ACCESS,
                )
            };
            recovery_handles[0].adopt_output(raw);
            duplicated.map_err(|cause| {
                phase.error(
                    "recovery_read_root_duplicate",
                    crate::claim_error::BoundedNativeCause::Windows(cause),
                )
            })?;
            if !recovery_handles[0].is_armed() {
                return Err(phase.invalid("recovery_read_root_output"));
            }
            let mut component = String::with_capacity(107);
            origin
                .write_name(&mut component)
                .map_err(|cause| ClaimError::new("recovery_read_name", cause))?;
            let generation = phase.name_destination()?.join(
                root.path()?,
                std::ffi::OsStr::new(&component),
                phase,
            )?;
            let mut indexes = [0; 3];
            for (level, leaf) in [None, Some("database"), Some("metrics")].into_iter().enumerate() {
                let path = if let Some(leaf) = leaf {
                    phase.name_destination()?.join(
                        generation.path()?,
                        std::ffi::OsStr::new(leaf),
                        phase,
                    )?
                } else {
                    generation.clone()
                };
                let opened = directories::open_relative(
                    recovery_handles,
                    &sid[..*sid_length],
                    budget.clone(),
                    directories::RelativeDirectory {
                        path: path.path()?,
                        parent: if level == 0 { 0 } else { indexes[0] },
                        mode: directories::Mode::Existing,
                        outputs: Some(directories::CountedDirectoryOutputs {
                            primary: Handle::new(HANDLE::default(), budget.clone()),
                            verification: Handle::new(HANDLE::default(), budget.clone()),
                            close_verification: Handle::close_in_phase,
                        }),
                    },
                    context,
                    phase,
                )?;
                indexes[level] = opened.index;
                let destination = phase.name_destination()?;
                let canonical =
                    paths::final_path(&recovery_handles[opened.index], phase, destination)?;
                if canonical.path()? != path.path()? {
                    return Err(phase.invalid("recovery_read_canonical_origin"));
                }
                if level != 0 {
                    identities[level - 1] = opened.identity;
                }
            }
            for index in 0..2 {
                std::mem::swap(&mut handles[index], &mut recovery_handles[indexes[index + 1]]);
            }
            for handle in recovery_handles.iter_mut().rev() {
                handle.close_in_phase(phase)?;
            }
            recovery_handles.clear();
            phase.context(context)
        })?;
        data.ready = true;
        owner.current(context)?;
        owner.current(context)
    }
}
