// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind concrete pipe/Event/Wait outputs to their exact admitted accounting owner.
use crate::{
    desktop_capture::resources::{CaptureEventClaim, CaptureEventOutput},
    native::NativeObservationError,
    native_resources::EffectContext,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{AdmissionError, NativeHandleClaim, NativeHandleOutput};

/// The two closed producer kinds share concrete native storage, not admission budgets.
#[derive(Clone)]
pub(crate) enum OutputAccounting {
    Symbol { handles: Arc<NativeHandleClaim>, original: EffectContext },
    Capture(CaptureEventClaim),
}
/// Preserve the exact original output guard through partial native failures.
pub(crate) enum OutputCredit {
    Symbol { _guard: NativeHandleOutput },
    Capture { _guard: CaptureEventOutput },
}
impl OutputAccounting {
    /// Retain original guard/port identity without creating a quota or native privilege.
    pub(crate) fn symbol(handles: Arc<NativeHandleClaim>, original: EffectContext) -> Self {
        Self::Symbol { handles, original }
    }
    /// Check actual original producer authority outside every owner lock.
    pub(crate) fn check(&self) -> Result<(), AdmissionError> {
        match self {
            Self::Symbol { original, .. } => original.check(),
            Self::Capture(original) => original.check(),
        }
    }
    /// Match the sealed original target before a fresh child cleanup effect.
    /// Successful returned closure is adopted before any later proof check.
    pub(crate) fn check_retirement(
        &self,
        proof: &crate::capture_bundle::CaptureBundleRetirement,
    ) -> Result<(), AdmissionError> {
        match self {
            Self::Capture(claim) if proof.matches_origin(claim.original()) => proof.check(),
            _ => Err(AdmissionError::InvalidCaptureAdmission),
        }
    }
    /// Identify the closed child owner kind without sampling a provider.
    pub(crate) fn capture(&self) -> bool {
        matches!(self, Self::Capture(_))
    }
    /// Reserve a typed output before native effects; keep truthful owner causes.
    pub(crate) fn reserve_output(
        &self,
        stage: &'static str,
    ) -> Result<OutputCredit, NativeObservationError> {
        match self {
            Self::Symbol { handles, .. } => handles
                .reserve_output()
                .map(|guard| OutputCredit::Symbol { _guard: guard })
                .map_err(|cause| NativeObservationError::new(stage, cause)),
            Self::Capture(handles) => handles
                .reserve_output()
                .map(|guard| OutputCredit::Capture { _guard: guard })
                .map_err(|cause| NativeObservationError::new(stage, cause)),
        }
    }
}
