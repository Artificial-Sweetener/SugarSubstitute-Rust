// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal one bounded development preview from the actual retained Desktop capture source.
//! Loans are inert until a real ordinary worker binds its original admission; neither
//! source nor seal provides a scheduler, native handle, fault privilege or COM object.
mod binding;
mod source;
use crate::{desktop_capture::metadata::DesktopCaptureCustodyRef, native::NativeObservationError};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{execution::*, startup::MainIdentity};

/// Preserve the original owner, counter conversion or bounded destination rejection.
#[derive(Debug)]
pub enum PreviewArmingError {
    /// Actual source/admission/cancellation/clock veto, with its original cause.
    Admission(AdmissionError),
    /// Original native counter sampling or immutable conversion failure.
    Native(NativeObservationError),
    /// Destination is not bounded D-drive output, or input was already consumed.
    Destination,
}
impl fmt::Display for PreviewArmingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(cause) => cause.fmt(f),
            Self::Native(cause) => cause.fmt(f),
            Self::Destination => f.write_str("Preview destination or original input is invalid"),
        }
    }
}
impl Error for PreviewArmingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Destination => None,
        }
    }
}
impl From<AdmissionError> for PreviewArmingError {
    fn from(cause: AdmissionError) -> Self {
        Self::Admission(cause)
    }
}
/// Move-only source loan minted from the authenticated Session on its composing UI thread.
/// It shares only the original precharged capture metadata. No new shared allocation,
/// weak observation, context, session or native capability is created. The Host includes
/// storage_bytes() in its existing concrete controller preflight before retaining it.
/// The campaign is decoded once here, from original enrollment+180s at most; worker
/// binding, reveal, getters and checked close never translate or renew that deadline.
#[must_use = "Retain the original loan until worker binding or explicit rejection"]
pub struct PreviewSourceLoan {
    custody: DesktopCaptureCustodyRef,
    run: sugarsubstitute_contracts::lifecycle::RunKey,
    native_scope: ScopeId,
    source_scope: ScopeId,
    control_bytes: usize,
    counter: CounterDeadline,
    until: Moment,
}
impl PreviewSourceLoan {
    /// Original immutable Host controller/path declaration accepted before source allocation.
    /// The Host rejects its concrete controller/path preflight if it exceeds this amount.
    /// This numeric observation grants no new claim or native authority.
    pub const fn control_storage_bytes(&self) -> usize {
        self.control_bytes
    }

    /// Concrete inline storage charged by the Host's existing controller preflight.
    pub const fn storage_bytes() -> usize {
        size_of::<Self>()
    }
    /// Return the once-converted original campaign cutoff in its original Core epoch.
    pub const fn original_deadline(&self) -> Moment {
        self.until
    }
    /// Return the unchanged original native campaign cutoff; this samples no provider.
    pub const fn counter_deadline(&self) -> CounterDeadline {
        self.counter
    }
}
/// Nonclone preview permission owned by the admitted persister and its failure terminal.
/// Field order frees path/source/context storage before returning the cumulative claim.
/// Checked close of already-owned filesystem outputs does not require renewed effects.
#[must_use = "Retain with every actual artifact output through checked physical closure"]
pub struct PreviewArmingSeal {
    loan: PreviewSourceLoan,
    destination: PathBuf,
    original: JobContext,
    identity: MainIdentity,
    _storage: MemoryClaim,
}
impl PreviewArmingSeal {
    /// Test-only original context observation; no consumer can construct a seal.
    #[cfg(test)]
    pub(crate) fn original_for_owner_fixture(&self) -> JobContext {
        self.original.clone()
    }
    /// Borrow the original bounded destination; no path copy or provider is invoked.
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    /// Return the identical fixed campaign cutoff used by the original JobSpec.
    pub const fn original_deadline(&self) -> Moment {
        self.loan.until
    }
    /// Return the unchanged original campaign counter, without reconversion.
    pub const fn counter_deadline(&self) -> CounterDeadline {
        self.loan.counter
    }
    /// Fence a fresh filesystem stage against this real worker and current source.
    /// # Errors
    /// Rejects foreign/current-changed identities or ledgers, stopped/retired source,
    /// owner shutdown, cancellation and either original or narrowed ancestor expiry.
    pub fn check(
        &self,
        context: &JobContext,
        identity: MainIdentity,
    ) -> Result<(), PreviewArmingError> {
        let core = source::core(&self.loan)?;
        {
            let state = crate::scope::lock(&core.state);
            crate::capture_bundle::validate::association(&core, &state, context)?;
        }
        let now = source::now(&core)?;
        let state = crate::scope::lock(&core.state);
        source::worker(&core, &state, &self.loan, context, now)?;
        let actual = crate::capture_bundle::validate::association(&core, &state, &self.original)?;
        if context.job_id() != self.original.job_id()
            || identity != self.identity
            || actual.memory.as_ref().is_none_or(|memory| !memory.matches_lease(context.memory()))
            || crate::scope::lock(&self.loan.custody.state).preview_job != Some(context.job_id())
        {
            return Err(AdmissionError::InvalidCaptureAdmission.into());
        }
        Ok(())
    }
}
