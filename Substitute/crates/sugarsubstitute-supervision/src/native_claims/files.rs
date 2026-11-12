// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain physical file outputs and counted custody through checked native retirement.
mod layout;
use crate::{ClaimError, claims::pool::SlotUse};
pub(crate) use layout::{NativeLayoutFailure, NativeRoleLayout};
use std::{
    fs::File,
    mem::ManuallyDrop,
    ops::Deref,
    os::windows::io::{FromRawHandle, IntoRawHandle},
    sync::Arc,
};
#[cfg(test)]
use sugarsubstitute_contracts::execution::NativeHandleOutput;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};

#[derive(Clone)]
pub(crate) enum Budget {
    Core(Arc<crate::claims::pool::Pool>),
    CustodyCore(crate::native_claims::backend_custody::control::WitnessControl),
    Slot(Arc<SlotUse>),
    Custody(crate::native_claims::backend_custody::pool::WitnessSlot),
    Protected(Arc<sugarsubstitute_contracts::execution::MemoryClaim>),
    Diagnostic(crate::diagnostic_storage::DiagnosticNativeGrant),
    SelectedRelease(crate::installed_release::AllocationOrigin),
}
impl Budget {
    /// Restrict helper namespace creation to its actual authenticated source.
    pub(crate) fn expected_namespace(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::NamespaceId> {
        match self {
            Self::CustodyCore(original) => original.expected_namespace(),
            _ => None,
        }
    }
    /// Apply actual custody controls without changing ordinary owners' contracts.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        match self {
            Self::CustodyCore(original) => original.check(),
            Self::Custody(slot) => slot.check(),
            _ => Ok(()),
        }
    }
    /// Require the same original custody allocation at borrowed file boundaries.
    pub(crate) fn matches_custody(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CustodyCore(a), Self::CustodyCore(b)) => a.same_allocation(b),
            (Self::Custody(a), Self::Custody(b)) => a.same_allocation(b),
            (Self::CustodyCore(_) | Self::Custody(_), _)
            | (_, Self::CustodyCore(_) | Self::Custody(_)) => false,
            _ => true,
        }
    }
    /// Restrict ordinary constructors to their three actual existing origins.
    pub(crate) fn is_ordinary(&self) -> bool {
        !matches!(self, Self::Diagnostic(_) | Self::SelectedRelease(_))
    }
    /// Reject an invalid origin inline for Diagnostic allocation custody.
    pub(crate) fn rejection(&self, operation: &'static str) -> ClaimError {
        match self {
            Self::Diagnostic(original) => {
                ClaimError::diagnostic_allocation(original.reject_native_state())
            }
            Self::SelectedRelease(_) => ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ),
            _ => ClaimError::invalid(operation),
        }
    }

    pub(crate) fn retain_failed(&self) {
        match self {
            Self::Core(value) => std::mem::forget(value.clone()),
            Self::CustodyCore(value) => std::mem::forget(value.clone()),
            Self::Slot(value) => std::mem::forget(value.clone()),
            Self::Custody(value) => std::mem::forget(value.clone()),
            Self::Protected(value) => std::mem::forget(value.clone()),
            Self::Diagnostic(value) => std::mem::forget(value.clone()),
            Self::SelectedRelease(value) => std::mem::forget(value.clone()),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Reserved,
    NoOutput,
    Adopted,
    Transferred,
    Closed,
}

enum OutputCredit {
    #[cfg(test)]
    Symbol {
        _output: NativeHandleOutput,
    },
    Capture {
        _output: crate::installed_release::CaptureOutputGuard,
    },
}

pub(crate) struct Handle {
    value: usize,
    budget: Budget,
    credit: Option<OutputCredit>,
    state: State,
    first_failure: Option<ClaimError>,
    secondary_failure: Option<ClaimError>,
}
impl Handle {
    /// Retain an existing uncounted owner's actual native output.
    pub(crate) fn new(value: HANDLE, budget: Budget) -> Self {
        Self {
            value: value.0 as usize,
            budget,
            credit: None,
            state: if value.0.is_null() { State::Closed } else { State::Adopted },
            first_failure: None,
            secondary_failure: None,
        }
    }
    /// Own an empty durable cell and its original accounting before the opener runs.
    #[cfg(test)]
    pub(crate) fn reserve_output(budget: Budget, credit: NativeHandleOutput) -> Self {
        Self {
            value: 0,
            budget,
            credit: Some(OutputCredit::Symbol { _output: credit }),
            state: State::Reserved,
            first_failure: None,
            secondary_failure: None,
        }
    }
    /// Borrow the original capture input until successful physical-cell reservation.
    /// # Errors
    /// Returns an inline cause without consuming a rejected original or output credit.
    pub(crate) fn reserve_capture(
        input: &mut Option<(
            crate::installed_release::AllocationOrigin,
            crate::installed_release::CaptureOutputGuard,
        )>,
    ) -> Result<Self, ClaimError> {
        let (original, credit) = input.as_ref().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        if !credit.original().matches(original) {
            return Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ));
        }
        let (original, credit) = input.take().ok_or_else(|| {
            ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            )
        })?;
        Ok(Self {
            value: 0,
            budget: Budget::SelectedRelease(original),
            credit: Some(OutputCredit::Capture { _output: credit }),
            state: State::Reserved,
            first_failure: None,
            secondary_failure: None,
        })
    }
    /// Adopt the producing API's actual output into its pre-reserved empty cell.
    /// The private opener calls this exactly once before inspecting native status.
    pub(crate) fn adopt_output(&mut self, raw: HANDLE) {
        if raw.0.is_null() || raw == INVALID_HANDLE_VALUE {
            self.value = 0;
            self.state = State::NoOutput;
        } else {
            self.value = raw.0 as usize;
            self.state = State::Adopted;
        }
    }
    /// Return an unused credit for an uninvoked or observed empty output.
    /// # Errors
    /// Rejects adopted/transferred output independently of its temporarily zero raw value.
    pub(crate) fn release_unused_output(&mut self) -> Result<(), ClaimError> {
        if self.value != 0
            || !matches!(self.state, State::Reserved | State::NoOutput | State::Closed)
        {
            return Err(self.budget.rejection("native_output_still_owned"));
        }
        self.state = State::Closed;
        self.credit = None;
        Ok(())
    }
    pub(crate) fn raw(&self) -> HANDLE {
        HANDLE(self.value as *mut _)
    }
    /// Transfer one actual uncounted custody output into its next durable native cell.
    /// This private adapter seam accepts no numeric caller factory or raw projection.
    pub(crate) fn take_custody_output(
        &mut self,
    ) -> Result<std::os::windows::io::OwnedHandle, ClaimError> {
        if !matches!(self.budget, Budget::CustodyCore(_))
            || self.credit.is_some()
            || self.state != State::Adopted
            || self.value == 0
        {
            return Err(self.budget.rejection("custody_transfer_output"));
        }
        let raw = std::mem::take(&mut self.value);
        self.state = State::Closed;
        // SAFETY: This cell uniquely owns the actual transferred output. Ownership
        // moves to one immediately installed process/Job cell under the same source.
        Ok(unsafe {
            <std::os::windows::io::OwnedHandle as FromRawHandle>::from_raw_handle(raw as *mut _)
        })
    }
    /// Observe physical custody even while a File borrow temporarily owns raw0.
    pub(crate) fn is_armed(&self) -> bool {
        matches!(self.state, State::Adopted | State::Transferred)
    }
    pub(crate) fn budget(&self) -> Budget {
        self.budget.clone()
    }
    /// Borrow the same actual file for the standard-library lock primitive.
    /// The original cell retains its credit and memory while raw ownership is borrowed.
    /// # Errors
    /// Rejects empty/closed/transferred cells before constructing a File.
    pub(crate) fn borrow_file(&mut self) -> Result<NativeFileBorrow<'_>, ClaimError> {
        if self.state != State::Adopted || self.value == 0 {
            return Err(self.budget.rejection("native_file_not_owned"));
        }
        let value = std::mem::take(&mut self.value);
        self.state = State::Transferred;
        // SAFETY: Unique raw ownership moves into this nonescaping borrow; Drop
        // restores the same output to its original cell before unwind propagation.
        let file = unsafe { File::from_raw_handle(value as *mut _) };
        Ok(NativeFileBorrow { parent: self, file: ManuallyDrop::new(file) })
    }
    /// Execute checked CloseHandle and release credit only after actual success.
    /// # Errors
    /// Retains raw output, bounded first/secondary causes, memory and credit on native error.
    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        if matches!(self.budget, Budget::SelectedRelease(_)) {
            return Err(self.budget.rejection("capture_close_requires_proof"));
        }
        if !self.budget.is_ordinary() {
            return Err(self.budget.rejection("native_close_requires_original_retirement"));
        }
        self.budget.check()?;
        self.close_using(|raw| {
            // SAFETY: Unique actual ownership remains armed through checked closure.
            unsafe { CloseHandle(raw) }.map_err(|error| ClaimError::new("close_handle", error))
        })
    }
    /// Close one exact normal-operation output under its original borrowed phase.
    /// Actual closure is adopted before any caller's post-return control observation.
    pub(crate) fn close_in_phase(
        &mut self,
        phase: &mut crate::durable_storage::protected::native_phase::NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        self.close_in_phase_using(phase, |raw| {
            // SAFETY: The same uniquely owned physical output remains armed.
            unsafe { CloseHandle(raw) }
        })
    }
    /// Check the actual running phase before one native close primitive.
    /// Uncertain returned closure retains this exact physical cell and its counted credit.
    pub(crate) fn close_in_phase_using(
        &mut self,
        phase: &mut crate::durable_storage::protected::native_phase::NativePhase<'_>,
        close: impl FnOnce(HANDLE) -> windows::core::Result<()>,
    ) -> Result<(), ClaimError> {
        phase.check_budget(&self.budget)?;
        phase.check()?;
        self.close_using(|raw| {
            close(raw).map_err(|cause| {
                phase.error("close_handle", crate::claim_error::BoundedNativeCause::Windows(cause))
            })
        })
    }
    /// Settle an existing Diagnostic output using actual accepted storage cleanup.
    /// The normal ledger is closed. Only preheld causes are used; the finite call's
    /// own successful return is adopted passively without a second provider/native call.
    pub(crate) fn close_diagnostic(
        &mut self,
        retirement: &mut crate::diagnostic_storage::DiagnosticNativeRetirement<'_>,
        causes: &mut crate::durable_storage::protected::native_file::NativeCauseDestinations,
    ) -> Result<(), ClaimError> {
        let Budget::Diagnostic(original) = &self.budget else {
            return Err(self.budget.rejection("diagnostic_retirement_origin"));
        };
        causes.matches_original(original)?;
        if !self.is_armed() && self.value == 0 {
            return self.release_unused_output();
        }
        causes.check(true)?;
        let observed = retirement.check(original).map_err(ClaimError::diagnostic_allocation);
        causes.retirement_control(observed)?;
        self.close_using(|raw| {
            // SAFETY: The original accepted close retains this exact live output.
            unsafe { CloseHandle(raw) }.map_err(|cause| {
                causes.error(
                    "close_handle",
                    crate::claim_error::BoundedNativeCause::Windows(cause),
                    true,
                )
            })
        })
    }
    /// Settle exactly this accepted capture output under the original borrowed close proof.
    pub(crate) fn close_selected(
        &mut self,
        proof: &crate::installed_release::SelectedRetirement<'_>,
        causes: &mut crate::durable_storage::protected::native_file::NativeCauseDestinations,
    ) -> Result<(), ClaimError> {
        let Budget::SelectedRelease(original) = &self.budget else {
            return Err(self.budget.rejection("capture_close_origin"));
        };
        causes.matches_selected(original)?;
        if !self.is_armed() && self.value == 0 {
            return self.release_unused_output();
        }
        if !proof.matches_origin(original.original()) {
            return Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ));
        }
        causes.check(true)?;
        if let Err(cause) = proof.check() {
            return Err(causes.selected_control(cause, true));
        }
        // Accept the exact call before cutoff; checked returned facts settle only
        // this output without another proof/provider/native call on the return path.
        self.close_using(|raw| {
            // SAFETY: The checked original cutoff admitted this exact still-owned raw output;
            // successful returned closure is adopted by the same durable cell.
            unsafe { CloseHandle(raw) }.map_err(|cause| {
                causes.error(
                    "capture_close_handle",
                    crate::claim_error::BoundedNativeCause::Windows(cause),
                    true,
                )
            })
        })
    }
    /// Apply checked external retirement to the same physical cell.
    /// Production supplies only actual CloseHandle; owner tests inject a failure
    /// while keeping a real output live. Only checked success returns its credit.
    fn close_using(
        &mut self,
        close: impl FnOnce(HANDLE) -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        if self.state == State::Transferred {
            return Err(self.budget.rejection("native_file_transferred"));
        }
        if self.value != 0 {
            if let Err(cause) = close(self.raw()) {
                if self.first_failure.is_none() {
                    self.first_failure = Some(cause.clone());
                } else if self.secondary_failure.is_none() {
                    self.secondary_failure = Some(cause.clone());
                }
                return Err(cause);
            }
            self.value = 0;
            self.state = State::Closed;
        }
        self.release_unused_output()
    }
}

/// Restore the actual borrowed File to its original cell on every unwind path.
pub(crate) struct NativeFileBorrow<'a> {
    parent: &'a mut Handle,
    file: ManuallyDrop<File>,
}
impl Deref for NativeFileBorrow<'_> {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        &self.file
    }
}
impl Drop for NativeFileBorrow<'_> {
    fn drop(&mut self) {
        // SAFETY: This borrow uniquely owns the File and takes it exactly once.
        let file = unsafe { ManuallyDrop::take(&mut self.file) };
        self.parent.value = file.into_raw_handle() as usize;
        self.parent.state = State::Adopted;
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if (self.credit.is_some()
            || !self.budget.is_ordinary()
            || matches!(self.budget, Budget::Custody(_) | Budget::CustodyCore(_)))
            && matches!(self.state, State::Adopted | State::Transferred)
        {
            let retained = Self {
                value: std::mem::take(&mut self.value),
                budget: self.budget.clone(),
                credit: self.credit.take(),
                state: self.state,
                first_failure: self.first_failure.take(),
                secondary_failure: self.secondary_failure.take(),
            };
            // Retain the complete physical cell before diagnostics. Guard Drop cannot
            // run; this fallback and logging perform no native close.
            std::mem::forget(retained);
            self.state = State::Closed;
            tracing::error!(
                operation = "native_output_abandoned",
                "Native output custody retained"
            );
        } else if !self.budget.is_ordinary() {
            // No actual output exists; this releases only the inert empty cell.
            self.state = State::Closed;
            self.credit = None;
        } else if let Err(cause) = self.close() {
            // Preserve the established uncounted owner contract.
            tracing::error!(operation = cause.operation(), "Native claim handle closure failed");
            self.budget.retain_failed();
        }
    }
}
pub(crate) fn close_all(handles: &mut Vec<Handle>) -> Result<(), ClaimError> {
    for handle in handles.iter_mut().rev() {
        handle.close()?;
    }
    handles.clear();
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
