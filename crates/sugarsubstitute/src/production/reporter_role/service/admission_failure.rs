// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move exact child admission causes without exporting pending frame or source custody.
use super::*;
use std::{error::Error, fmt};
#[derive(Debug)]
enum Cause {
    Admission(AdmissionError),
    Identity(IdentityError),
}
/// The original source/input remains in its service until actual transport disposal.
/// This owner retains only the exact error and its original finite reservation credit.
#[derive(Debug)]
pub(in crate::production::reporter_role) struct OwnedViewAdmissionFailure {
    cause: Cause,
    _original: MemoryLease,
}
impl fmt::Display for OwnedViewAdmissionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reporter view admission failed")
    }
}
impl Error for OwnedViewAdmissionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match &self.cause {
            Cause::Admission(cause) => cause,
            Cause::Identity(cause) => cause,
        })
    }
}
impl ViewService {
    /// Transfer once after actual admission rejection; immutable fence prevents renewed work.
    /// No completion is created and original input ownership stays in the funded source.
    pub(in crate::production::reporter_role) fn take_admission_failure(
        &mut self,
    ) -> Option<OwnedViewAdmissionFailure> {
        if self.admission_transferred
            || !self.admitted_input
            || self.job.is_some()
            || self.completion.is_some()
        {
            return None;
        }
        let cause = if let Some(cause) = self.admission.take() {
            Cause::Admission(cause)
        } else {
            Cause::Identity(self.identity.take()?)
        };
        self.admission_transferred = true;
        Some(OwnedViewAdmissionFailure { cause, _original: self.source.retained_memory() })
    }
}
#[cfg(test)]
#[path = "admission_failure/tests.rs"]
mod tests;
