// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Duplicate only the actual retained protected generation's database and metrics pins.
use super::*;
use windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE},
    System::Threading::GetCurrentProcess,
};
impl ProtectedStorageRoot {
    pub(crate) fn prepare_capture_read_handles(
        &self,
        owner: &mut CaptureReadOwner,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        owner.current(context)?;
        let source =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_source_closed"))?;
        let generation = source
            .generation
            .as_ref()
            .filter(|generation| generation.ready)
            .ok_or_else(|| source.budget.rejection("capture_read_generation_unavailable"))?;
        let data =
            owner.data.as_mut().ok_or_else(|| source.budget.rejection("capture_read_closed"))?;
        if data.coordinator != generation.coordinator || data.identities != generation.identities {
            return Err(source.budget.rejection("capture_read_generation_changed"));
        }
        let CaptureReadData {
            handles,
            identities,
            sid,
            sid_length,
            causes,
            original,
            pin: source_pin,
            ..
        } = data.as_mut();
        let budget = Budget::Diagnostic(original.clone());
        resolution::with_phase(&budget, causes, context, resolution::ScratchRole::Path, |phase| {
            phase.capture_source(source_pin)?;
            for index in 0..2 {
                let pin = source
                    .handles
                    .get(generation.slots[index])
                    .ok_or_else(|| phase.invalid("capture_read_source_pin"))?;
                phase.context(context)?;
                paths::validate_object(pin, true, true, phase)?;
                if paths::file_identity(pin, phase)? != identities[index] {
                    return Err(phase.invalid("capture_read_source_changed"));
                }
                security::verify(pin, &sid[..*sid_length], phase)?;
                if handles[index].is_armed() {
                    return Err(phase.invalid("capture_read_duplicate_attempt"));
                }
                phase.context(context)?;
                let mut output = HANDLE::default();
                // SAFETY: The source directory handle stays pinned by the borrowed root
                // and original read pin. The initialized synchronous output is adopted
                // before status inspection, including partial native failure.
                let duplicated = unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        pin.raw(),
                        GetCurrentProcess(),
                        &mut output,
                        0,
                        false,
                        DUPLICATE_SAME_ACCESS,
                    )
                };
                handles[index].adopt_output(output);
                duplicated.map_err(|cause| {
                    phase.error(
                        "capture_read_duplicate",
                        crate::claim_error::BoundedNativeCause::Windows(cause),
                    )
                })?;
                if !handles[index].is_armed() {
                    return Err(phase.invalid("capture_read_duplicate_output"));
                }
                if paths::file_identity(&handles[index], phase)? != identities[index] {
                    return Err(phase.invalid("capture_read_duplicate_changed"));
                }
                security::verify(&handles[index], &sid[..*sid_length], phase)?;
                phase.context(context)?;
            }
            Ok(())
        })?;
        data.ready = true;
        owner.current(context)
    }
}
impl CaptureReadOwner {
    pub(crate) fn validate_handles(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.current(context)?;
        let data = self.data.as_mut().ok_or_else(|| ClaimError::invalid("capture_read_closed"))?;
        if !data.ready {
            return Err(ClaimError::diagnostic_allocation(data.original.reject_native_state()));
        }
        let CaptureReadData { handles, identities, sid, sid_length, causes, original, .. } =
            data.as_mut();
        let budget = Budget::Diagnostic(original.clone());
        resolution::with_phase(&budget, causes, context, resolution::ScratchRole::Path, |phase| {
            for index in 0..2 {
                paths::validate_object(&handles[index], true, true, phase)?;
                if paths::file_identity(&handles[index], phase)? != identities[index] {
                    return Err(phase.invalid("capture_read_current_identity"));
                }
                security::verify(&handles[index], &sid[..*sid_length], phase)?;
            }
            Ok(())
        })
    }
}

impl CaptureReadOwner {
    pub(crate) fn borrowed(
        &self,
        index: usize,
    ) -> Result<std::os::windows::io::BorrowedHandle<'_>, ClaimError> {
        let data = self.data.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_closed"))?;
        let handle =
            data.handles.get(index).filter(|handle| handle.is_armed()).ok_or_else(|| {
                ClaimError::diagnostic_allocation(data.original.reject_native_state())
            })?;
        // SAFETY: This borrow retains the exact checked directory owner and prevents
        // mutable native retirement throughout the synchronous foreign call.
        Ok(unsafe { std::os::windows::io::BorrowedHandle::borrow_raw(handle.raw().0) })
    }
    pub(crate) fn retire(
        &mut self,
        control: &mut crate::diagnostic_storage::DiagnosticNativeRetirement<'_>,
    ) -> Result<(), ClaimError> {
        if let Some(data) = self.data.as_mut() {
            for handle in &mut data.handles {
                handle.close_diagnostic(control, &mut data.causes)?;
            }
            data.ready = false;
        }
        // The actual native cells and source pin drop before their outside current credit.
        drop(self.data.take());
        drop(self.allocation.take());
        Ok(())
    }
}
