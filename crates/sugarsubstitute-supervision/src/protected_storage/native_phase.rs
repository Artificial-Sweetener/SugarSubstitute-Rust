// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow one closed admitted native scratch session across its actual buffers.
mod buffers;
use super::native_name::{NativeNameDestination, NativeSidDestination};
use super::{native_file::NativeCauseDestinations, paths, security};
use crate::{
    ClaimError,
    claim_error::BoundedNativeCause,
    diagnostic_storage::{DiagnosticNativeGrant, DiagnosticNativeScratchLoan},
    native_claims::files::Budget,
};
pub(crate) use buffers::{NativeOpenBuffers, NativeWidePath};
use std::{os::windows::ffi::OsStrExt, path::Path};
use sugarsubstitute_contracts::execution::JobContext;

/// Keep one actual top-role allocation and preheld cause pair exclusive.
pub(crate) enum NativePhase<'a> {
    Ordinary {
        budget: &'a Budget,
    },
    Selected {
        original: &'a crate::installed_release::AllocationOrigin,
        scratch: &'a mut crate::installed_release::CaptureScratch,
        causes: std::cell::RefCell<&'a mut NativeCauseDestinations>,
    },
    Diagnostic {
        original: &'a DiagnosticNativeGrant,
        current: &'a JobContext,
        scratch: &'a mut DiagnosticNativeScratchLoan,
        causes: &'a mut NativeCauseDestinations,
        source: Option<super::capture_read::pin::ReadPins>,
    },
}
impl<'origin> NativePhase<'origin> {
    pub(crate) fn selected_name_error(
        &mut self,
        cause: crate::installed_release::names::CaptureNameFailure,
    ) -> ClaimError {
        match self {
            Self::Selected { causes, .. } => causes.get_mut().selected_error(
                "capture_name",
                crate::installed_release::CaptureBoundedCause::Name(cause),
                false,
            ),
            _ => {
                std::mem::forget(cause);
                ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::identity(),
                )
            }
        }
    }

    /// Reborrow this SAME admitted scratch with the physical cell's preheld causes.
    /// Neither origin nor capacity is reconstructed from the caller's values.
    pub(crate) fn for_causes<'cell>(
        &'cell mut self,
        causes: &'cell mut NativeCauseDestinations,
    ) -> Result<NativePhase<'cell>, ClaimError> {
        self.check()?;
        match self {
            Self::Ordinary { budget } => NativePhase::ordinary(budget),
            Self::Selected { original, scratch, .. } => {
                causes.matches_selected(original)?;
                Ok(NativePhase::Selected {
                    original,
                    scratch,
                    causes: std::cell::RefCell::new(causes),
                })
            }
            Self::Diagnostic { original, current, scratch, source, .. } => {
                causes.matches_original(original)?;
                Ok(NativePhase::Diagnostic {
                    original,
                    current,
                    scratch,
                    causes,
                    source: source.as_ref().map(super::capture_read::pin::ReadPins::borrow),
                })
            }
        }
    }
    /// Bind the actual retained source pin while current buffers/causes stay charged.
    /// Only the protected capture receiver can supply this constructor-private pin.
    pub(in crate::protected_storage) fn capture_source(
        &mut self,
        pin: &super::capture_read::pin::ReadPin,
    ) -> Result<(), ClaimError> {
        self.check()?;
        match self {
            Self::Diagnostic { source, .. } if pin.open() => {
                *source = Some(pin.fence());
                Ok(())
            }
            _ => Err(ClaimError::invalid("native_phase_source_origin")),
        }
    }
    /// Borrow an ordinary owner only after excluding every closed diagnostic origin.
    pub(crate) fn ordinary(budget: &'origin Budget) -> Result<Self, ClaimError> {
        if !budget.is_ordinary() {
            return Err(budget.rejection("native_phase_ordinary_origin"));
        }
        Ok(Self::Ordinary { budget })
    }
    /// Validate original allocation identity before allocating or native output.
    pub(crate) fn check_budget(&self, budget: &Budget) -> Result<(), ClaimError> {
        match (self, budget) {
            (Self::Ordinary { budget: original }, current)
                if original.is_ordinary()
                    && current.is_ordinary()
                    && original.matches_custody(current) =>
            {
                Ok(())
            }
            (Self::Diagnostic { original, .. }, Budget::Diagnostic(current))
                if original.matches_original(current) =>
            {
                self.check()
            }
            (Self::Diagnostic { source: Some(pin), .. }, source)
                if pin.matches_budget(source) && pin.open() =>
            {
                self.check()
            }
            (Self::Selected { original, .. }, Budget::SelectedRelease(current))
                if original.matches(current) =>
            {
                self.check()
            }
            _ => Err(budget.rejection("native_phase_changed_origin")),
        }
    }
    /// Revalidate real first/current operation controls without granting OS authority.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        match self {
            Self::Selected { original, scratch, causes } => {
                if !scratch.original().matches(original) {
                    return Err(ClaimError::capture_allocation(
                        crate::installed_release::CaptureCapacityFailure::identity(),
                    ));
                }
                let observed = original.original().check_effect();
                let mut causes = causes.try_borrow_mut().map_err(|_| {
                    ClaimError::capture_allocation(
                        crate::installed_release::CaptureCapacityFailure::missing(),
                    )
                })?;
                causes.matches_selected(original)?;
                match observed {
                    Ok(()) => causes.check(false),
                    Err(cause) => Err(causes.selected_control(cause, false)),
                }
            }
            Self::Ordinary { budget } if budget.is_ordinary() => budget.check(),
            Self::Ordinary { budget } => Err(budget.rejection("native_phase_ordinary_origin")),
            Self::Diagnostic { original, current, causes, .. } => {
                original.check(current).map_err(ClaimError::diagnostic_allocation)?;
                causes.check(false)
            }
        }
    }
    /// Observe original control after a finite return, including failed native calls.
    /// This never authorizes another effect or requires an unused first cause slot.
    pub(crate) fn post_context<T>(
        &mut self,
        result: Result<T, ClaimError>,
        current: &JobContext,
    ) -> Result<T, ClaimError> {
        match self {
            Self::Ordinary { budget } if !budget.is_ordinary() => {
                Err(budget.rejection("native_phase_ordinary_origin"))
            }
            Self::Ordinary { .. } => ClaimError::finish(result, super::check(current)),
            Self::Selected { original, causes, .. } => {
                causes.get_mut().selected_post(result, original)
            }
            Self::Diagnostic { original, causes, .. } => {
                let observed = original.check(current).map_err(ClaimError::diagnostic_allocation);
                causes.finish_observation(result, observed)
            }
        }
    }
    /// Observe original control using the same physical cell's preheld cause pair.
    /// No scratch is admitted or borrowed again after the producing scope returns.
    pub(crate) fn post_context_for<T>(
        &mut self,
        result: Result<T, ClaimError>,
        current: &JobContext,
        causes: &mut NativeCauseDestinations,
    ) -> Result<T, ClaimError> {
        match self {
            Self::Ordinary { budget } if !budget.is_ordinary() => {
                Err(budget.rejection("native_phase_ordinary_origin"))
            }
            Self::Ordinary { .. } => ClaimError::finish(result, super::check(current)),
            Self::Selected { original, .. } => causes.selected_post(result, original),
            Self::Diagnostic { original, .. } => {
                // Observe actual original control even if the cell binding is invalid.
                let observed = original.check(current).map_err(ClaimError::diagnostic_allocation);
                let observed = observed.and_then(|()| causes.matches_original(original));
                causes.finish_observation(result, observed)
            }
        }
    }
    /// Observe the internally retained current operation after adopting finite output.
    /// Ordinary exclusion acquisition has no added job-context contract.
    pub(crate) fn post_current_for<T>(
        &mut self,
        result: Result<T, ClaimError>,
        causes: &mut NativeCauseDestinations,
    ) -> Result<T, ClaimError> {
        match self {
            Self::Ordinary { budget } if !budget.is_ordinary() => {
                Err(budget.rejection("native_phase_ordinary_origin"))
            }
            Self::Ordinary { .. } => result,
            Self::Selected { original, .. } => causes.selected_post(result, original),
            Self::Diagnostic { original, current, .. } => {
                let observed = original.check(current).map_err(ClaimError::diagnostic_allocation);
                let observed = observed.and_then(|()| causes.matches_original(original));
                causes.finish_observation(result, observed)
            }
        }
    }
    /// Borrow the already admitted path overlap; no second reservation is made.
    pub(crate) fn path(&mut self) -> Result<(), ClaimError> {
        self.check()?;
        if let Self::Diagnostic { scratch, current, .. } = self {
            scratch
                .borrow_path()
                .map_err(ClaimError::diagnostic_allocation)?
                .check(current)
                .map_err(ClaimError::diagnostic_allocation)?;
        }
        Ok(())
    }
    /// Borrow descriptor/query overlap from this same admitted top role.
    pub(crate) fn security(&mut self) -> Result<(), ClaimError> {
        self.check()?;
        if let Self::Diagnostic { scratch, current, .. } = self {
            scratch
                .borrow_security()
                .map_err(ClaimError::diagnostic_allocation)?
                .check(current)
                .map_err(ClaimError::diagnostic_allocation)?;
        }
        Ok(())
    }
    /// Check the closed account-query scratch before its original token buffer.
    pub(crate) fn account(&self) -> Result<(), ClaimError> {
        self.check()?;
        if let Self::Diagnostic { scratch, current, .. } = self {
            scratch.check_account(current).map_err(ClaimError::diagnostic_allocation)?;
        }
        Ok(())
    }
    /// Admit the complete SID copy/control before querying its producing token.
    pub(crate) fn sid_destination(&self) -> Result<NativeSidDestination, ClaimError> {
        self.account()?;
        match self {
            Self::Selected { original, .. } => original
                .sid_destination()
                .map(NativeSidDestination::Selected)
                .map_err(ClaimError::capture_allocation),
            Self::Ordinary { .. } => Ok(NativeSidDestination::Ordinary),
            Self::Diagnostic { original, current, .. } => Ok(NativeSidDestination::Diagnostic {
                allocation: original
                    .account_sid(current)
                    .map_err(ClaimError::diagnostic_allocation)?,
                original: (*original).clone(),
            }),
        }
    }
    /// Reject a SID copy destination from any other original ledger.
    pub(crate) fn check_sid_destination(
        &self,
        destination: &NativeSidDestination,
    ) -> Result<(), ClaimError> {
        if let Self::Selected { original, .. } = self {
            return match destination {
                NativeSidDestination::Selected(current) if original.matches(current.original()) => {
                    self.account()
                }
                _ => Err(ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::identity(),
                )),
            };
        }
        match (self, destination.original()) {
            (Self::Ordinary { budget }, None) if budget.is_ordinary() => Ok(()),
            (Self::Diagnostic { original, .. }, Some(destination))
                if original.matches_original(destination) =>
            {
                self.account()
            }
            (Self::Diagnostic { original, .. }, _) => {
                Err(ClaimError::diagnostic_allocation(original.reject_native_state()))
            }
            (Self::Selected { .. }, _) => Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            )),
            (Self::Ordinary { budget }, _) => {
                Err(budget.rejection("native_sid_destination_origin"))
            }
        }
    }
    /// Preadmit a final name separately from this borrowed transient path peak.
    pub(crate) fn name_destination(&self) -> Result<NativeNameDestination, ClaimError> {
        self.check()?;
        match self {
            Self::Selected { original, scratch, .. } => {
                Ok(NativeNameDestination::SelectedTransient {
                    allocation: scratch.allocation(),
                    original: (*original).clone(),
                })
            }
            Self::Ordinary { .. } => Ok(NativeNameDestination::Ordinary),
            Self::Diagnostic { original, current, .. } => Ok(NativeNameDestination::Diagnostic {
                data: original.owned_name(current).map_err(ClaimError::diagnostic_allocation)?,
                control: original.namespace_control().map_err(ClaimError::diagnostic_allocation)?,
                original: (*original).clone(),
            }),
        }
    }
    /// Hold the actual 64KiB hash peak before its Vec can allocate.
    pub(crate) fn hash_destination(
        &self,
    ) -> Result<Option<DiagnosticNativeScratchLoan>, ClaimError> {
        self.check()?;
        match self {
            Self::Selected { .. } => Ok(None),
            Self::Ordinary { .. } => Ok(None),
            Self::Diagnostic { original, current, .. } => {
                original.native_hash(current).map(Some).map_err(ClaimError::diagnostic_allocation)
            }
        }
    }
    /// Reject a changed or disguised destination before decoder allocation.
    pub(crate) fn check_destination(
        &self,
        destination: &NativeNameDestination,
    ) -> Result<(), ClaimError> {
        if let Self::Selected { original, .. } = self {
            return match destination {
                NativeNameDestination::SelectedTransient { original: current, .. }
                    if original.matches(current) =>
                {
                    self.check()
                }
                _ => Err(ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::identity(),
                )),
            };
        }
        match (self, destination.original()) {
            (Self::Ordinary { budget }, None) if budget.is_ordinary() => Ok(()),
            (Self::Diagnostic { original, .. }, Some(destination))
                if original.matches_original(destination) =>
            {
                self.check()
            }
            (Self::Diagnostic { original, .. }, _) => {
                Err(ClaimError::diagnostic_allocation(original.reject_native_state()))
            }
            (Self::Selected { .. }, _) => Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            )),
            (Self::Ordinary { budget }, _) => {
                Err(budget.rejection("native_name_destination_origin"))
            }
        }
    }
    /// Preserve one bounded genuine external leaf in this preheld destination.
    pub(crate) fn error(
        &mut self,
        operation: &'static str,
        cause: BoundedNativeCause,
    ) -> ClaimError {
        match self {
            Self::Ordinary { budget } if budget.is_ordinary() => cause.ordinary_error(operation),
            Self::Ordinary { budget } => budget.rejection("native_phase_ordinary_origin"),
            Self::Diagnostic { causes, .. } => causes.error(operation, cause, false),
            Self::Selected { causes, .. } => causes.get_mut().error(operation, cause, false),
        }
    }
    /// Preserve a finite CNG status in the admitted bounded cause destination.
    /// Diagnostic failure conversion uses code-only HRESULT state; ambient COM
    /// error information cannot become an unaccounted retained native cause.
    /// Ordinary conversion preserves the existing Windows error behavior.
    pub(crate) fn ntstatus(
        &mut self,
        operation: &'static str,
        status: windows::Win32::Foundation::NTSTATUS,
    ) -> Result<(), ClaimError> {
        if let Self::Ordinary { budget } = self
            && !budget.is_ordinary()
        {
            return Err(budget.rejection("native_phase_ordinary_origin"));
        }
        if status.is_ok() {
            return Ok(());
        }
        let cause = match self {
            Self::Ordinary { .. } => status.ok().err(),
            Self::Diagnostic { .. } | Self::Selected { .. } => {
                Some(windows::core::Error::from_hresult(status.to_hresult()))
            }
        };
        match cause {
            Some(cause) => Err(self.error(operation, BoundedNativeCause::Windows(cause))),
            None => Ok(()),
        }
    }
    /// Preserve the original InvalidData leaf without a new uncharged native Arc.
    pub(crate) fn invalid(&mut self, operation: &'static str) -> ClaimError {
        self.error(
            operation,
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::InvalidData)),
        )
    }
    /// Check cancellation and the existing actual job deadline before each effect.
    pub(crate) fn context(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.check()?;
        if matches!(self, Self::Selected { .. }) {
            return Ok(());
        }
        if context.cancellation_requested()
            || context.deadline().is_none_or(|limit| context.now() >= limit)
        {
            return Err(self.invalid("storage_context_expired"));
        }
        Ok(())
    }
    fn path_units(&mut self, path: &Path) -> Result<Vec<u16>, ClaimError> {
        paths::validate_local_shape(path).map_err(|operation| self.invalid(operation))?;
        let mut units = Vec::new();
        let count = path.as_os_str().encode_wide().count() + 1;
        units.try_reserve_exact(count).map_err(|cause| {
            self.error("path_units_reservation", BoundedNativeCause::Reserve(cause))
        })?;
        units.extend(path.as_os_str().encode_wide());
        units.push(0);
        if units.capacity().checked_mul(size_of::<u16>()).is_none_or(|bytes| bytes > 65_534) {
            return Err(self.invalid("path_units_capacity"));
        }
        Ok(units)
    }
    /// Retain a producing units buffer and its exclusive phase borrow until adoption.
    pub(crate) fn wide<'phase>(
        &'phase mut self,
        path: &Path,
    ) -> Result<NativeWidePath<'phase, 'origin>, ClaimError> {
        self.path()?;
        let units = self.path_units(path)?;
        self.check()?;
        Ok(NativeWidePath { units, session: self })
    }
    /// Build the joint units/descriptor overlap within this one exclusive session.
    pub(crate) fn open_buffers<'phase>(
        &'phase mut self,
        path: &Path,
        sid: Option<&[u8]>,
    ) -> Result<NativeOpenBuffers<'phase, 'origin>, ClaimError> {
        self.path()?;
        let units = self.path_units(path)?;
        let descriptor =
            if let Some(sid) = sid { Some(security::Descriptor::new(sid, self)?) } else { None };
        self.check()?;
        Ok(NativeOpenBuffers { units, descriptor, _session: self })
    }
    /// Retain one relative NT name and descriptor in this same producing phase.
    pub(crate) fn relative_buffers<'phase>(
        &'phase mut self,
        child: &std::ffi::OsStr,
        sid: &[u8],
    ) -> Result<NativeOpenBuffers<'phase, 'origin>, ClaimError> {
        self.path()?;
        let count = child.encode_wide().count();
        if count == 0
            || count > 32766
            || child.encode_wide().any(|unit| matches!(unit, 0 | 47 | 58 | 92))
        {
            return Err(self.invalid("directory_leaf_name"));
        }
        let mut units = Vec::new();
        units.try_reserve_exact(count).map_err(|cause| {
            self.error("relative_units_reservation", BoundedNativeCause::Reserve(cause))
        })?;
        if units.capacity().checked_mul(size_of::<u16>()).is_none_or(|bytes| bytes > 65534) {
            return Err(self.invalid("relative_units_capacity"));
        }
        units.extend(child.encode_wide());
        let descriptor = Some(security::Descriptor::new(sid, self)?);
        self.check()?;
        Ok(NativeOpenBuffers { units, descriptor, _session: self })
    }
}
