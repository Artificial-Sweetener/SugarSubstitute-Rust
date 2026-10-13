// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preadmit and retain the physical owner's bounded first and secondary causes.
use super::*;
/// Hold both bounded genuine error destinations before any producing effect.
pub(crate) struct NativeCauseDestinations {
    first: Option<NativeCauseDestination>,
    secondary: Option<NativeCauseDestination>,
    original: Option<DiagnosticNativeGrant>,
    failure: Option<ClaimError>,
    selected: Option<crate::installed_release::AllocationOrigin>,
}
impl NativeCauseDestinations {
    /// Reject a substituted cause destination before borrowing an effect session.
    pub(crate) fn matches_original(&self, grant: &DiagnosticNativeGrant) -> Result<(), ClaimError> {
        if self.original.as_ref().is_some_and(|original| original.matches_original(grant)) {
            Ok(())
        } else {
            Err(ClaimError::diagnostic_allocation(grant.reject_native_state()))
        }
    }
    /// Admit both fixed cause controls before their actual external boundary.
    pub(crate) fn diagnostic(original: &DiagnosticNativeGrant) -> Result<Self, ClaimError> {
        Self::admit(original).map_err(ClaimError::diagnostic_allocation)
    }
    /// Admit the pair with an inline allocation rejection before any owner Box.
    pub(crate) fn admit(
        original: &DiagnosticNativeGrant,
    ) -> Result<Self, crate::diagnostic_storage::DiagnosticNativeAllocationFailure> {
        let first = original.native_cause()?;
        let secondary = original.native_secondary_cause()?;
        Ok(Self {
            first: Some(NativeCauseDestination::diagnostic(first, original.clone())),
            secondary: Some(NativeCauseDestination::diagnostic(secondary, original.clone())),
            original: Some(original.clone()),
            failure: None,
            selected: None,
        })
    }
    /// Consume both genuine capture destinations before the physical owner allocates.
    pub(crate) fn selected(
        original: &crate::installed_release::AllocationOrigin,
        first: NativeCauseDestination,
        secondary: NativeCauseDestination,
    ) -> Self {
        Self {
            first: Some(first),
            secondary: Some(secondary),
            original: None,
            failure: None,
            selected: Some(original.clone()),
        }
    }
    pub(crate) fn matches_selected(
        &self,
        original: &crate::installed_release::AllocationOrigin,
    ) -> Result<(), ClaimError> {
        if self.selected.as_ref().is_some_and(|held| held.matches(original)) {
            Ok(())
        } else {
            Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ))
        }
    }
    pub(crate) fn selected_error(
        &mut self,
        operation: &'static str,
        cause: crate::installed_release::CaptureBoundedCause,
        cleanup: bool,
    ) -> ClaimError {
        let destination = if cleanup && self.failure.is_some() {
            self.secondary.take()
        } else {
            self.first.take()
        };
        let Some(destination) = destination else {
            std::mem::forget(cause);
            return self.failure.clone().unwrap_or_else(|| {
                ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::missing(),
                )
            });
        };
        let observed = ClaimError::selected_native(operation, cause, destination);
        let observed = match self.failure.take() {
            Some(first) if cleanup => first.with_native_secondary(observed),
            Some(first) => {
                self.failure = Some(first);
                observed
            }
            None => observed,
        };
        if cleanup || self.failure.is_none() {
            self.failure = Some(observed.clone());
        }
        observed
    }
    pub(crate) fn selected_control(
        &mut self,
        cause: sugarsubstitute_contracts::execution::AdmissionError,
        cleanup: bool,
    ) -> ClaimError {
        self.selected_error(
            "capture_original_control",
            crate::installed_release::CaptureBoundedCause::Admission(cause),
            cleanup,
        )
    }
    pub(crate) fn selected_post<T>(
        &mut self,
        result: Result<T, ClaimError>,
        original: &crate::installed_release::AllocationOrigin,
    ) -> Result<T, ClaimError> {
        match original.original().check_effect() {
            Ok(()) => result,
            Err(cause) => {
                let already_retained = result
                    .as_ref()
                    .err()
                    .zip(self.failure.as_ref())
                    .is_some_and(|(first, held)| first.same_primary(held));
                let observed = self.selected_control(cause, true);
                match result {
                    Err(first) if !already_retained => Err(first.with_native_secondary(observed)),
                    _ => Err(observed),
                }
            }
        }
    }
    pub(crate) fn ordinary() -> Self {
        Self { first: None, secondary: None, original: None, failure: None, selected: None }
    }
    /// Preserve post-return control failure behind the genuine first native cause.
    /// The fixed secondary destination was admitted before the original effect.
    pub(crate) fn finish_observation<T>(
        &mut self,
        result: Result<T, ClaimError>,
        observed: Result<(), ClaimError>,
    ) -> Result<T, ClaimError> {
        let result = match (result, observed) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(cause), Err(secondary)) => {
                if let Some(destination) = self.secondary.take() {
                    ClaimError::finish_native::<T>(Err(cause), Err(secondary), destination)
                } else {
                    // An already exhausted owner cannot admit a third cause graph.
                    // Keep unexpected escaped custody before reporting the invariant.
                    std::mem::forget(secondary);
                    tracing::error!(
                        operation = "native_post_control_exhausted",
                        "Additional control custody retained"
                    );
                    Err(cause)
                }
            }
        };
        if let Err(cause) = &result {
            self.failure = Some(cause.clone());
        }
        result
    }
    /// Reject a spent native destination before another external effect.
    pub(crate) fn check(&self, cleanup: bool) -> Result<(), ClaimError> {
        if self.original.is_some() || self.selected.is_some() {
            let available = if cleanup && self.failure.is_some() {
                self.secondary.is_some()
            } else {
                self.first.is_some()
            };
            if !available {
                return Err(self.failure.clone().unwrap_or_else(|| {
                    self.original.as_ref().map_or_else(
                        || {
                            ClaimError::capture_allocation(
                                crate::installed_release::CaptureCapacityFailure::missing(),
                            )
                        },
                        |original| {
                            ClaimError::diagnostic_allocation(original.reject_native_state())
                        },
                    )
                }));
            }
        }
        Ok(())
    }
    /// Preserve a concrete genuine error using the already held first/secondary slot.
    pub(crate) fn error(
        &mut self,
        operation: &'static str,
        cause: BoundedNativeCause,
        cleanup: bool,
    ) -> ClaimError {
        let destination = if cleanup && self.failure.is_some() {
            self.secondary.take()
        } else {
            self.first.take()
        };
        if let Some(destination) = destination {
            let error = ClaimError::native(operation, cause, destination);
            let error = match self.failure.take() {
                Some(first) if cleanup => first.with_native_secondary(error),
                Some(first) => {
                    self.failure = Some(first);
                    error
                }
                None => error,
            };
            if cleanup || self.failure.is_none() {
                self.failure = Some(error.clone());
            }
            error
        } else if self.original.is_some() || self.selected.is_some() {
            // Effects reject a spent destination. An unexpected extra leaf may
            // itself retain data/control custody; preserve it before diagnostics.
            std::mem::forget(cause);
            tracing::error!(operation, "Unexpected additional native cause custody retained");
            self.failure.clone().unwrap_or_else(|| {
                self.original.as_ref().map_or_else(
                    || {
                        ClaimError::capture_allocation(
                            crate::installed_release::CaptureCapacityFailure::missing(),
                        )
                    },
                    |original| ClaimError::diagnostic_allocation(original.reject_native_state()),
                )
            })
        } else {
            cause.ordinary_error(operation)
        }
    }
    /// Preserve actual cleanup-control rejection behind any original native failure.
    /// The caller supplies the real borrowed retirement producer's observation.
    pub(crate) fn retirement_control(
        &mut self,
        observed: Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        match observed {
            Ok(()) => Ok(()),
            Err(cause) => {
                let previous = self.failure.clone();
                match previous {
                    Some(first) => self.finish_observation(Err(first), Err(cause)),
                    None => self.finish_observation(Ok(()), Err(cause)),
                }
            }
        }
    }
}
