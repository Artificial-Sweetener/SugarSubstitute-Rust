// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep closed preparation facts and both real diagnostic/application failures together.
use super::*;
impl ProductionChild {
    pub(super) fn observe_diagnostics<T>(&self, result: Result<T>) -> Result<T> {
        if self.diagnostics.check().is_ok() {
            return result;
        }
        crate::diagnostics::resolve(result, &self.diagnostics).map_err(|cause| {
            Box::new(PreparationDiagnosticFailure {
                cause,
                preparation: self.bootstrap.preparation_observation(),
            }) as Box<dyn Error>
        })
    }
}
/// Borrow the diagnostic or application cause a preparation diagnostic failure retained.
pub(super) fn preparation_cause<'a>(
    cause: &'a (dyn Error + 'static),
) -> Option<&'a (dyn Error + 'static)> {
    cause.downcast_ref::<PreparationDiagnosticFailure>().map(|failure| failure.cause.as_ref())
}
/// Keep real closed bootstrap predicates alongside the original diagnostic/application cause.
#[derive(Debug)]
struct PreparationDiagnosticFailure {
    cause: Box<dyn Error>,
    preparation: bootstrap::PreparationObservation,
}
impl std::fmt::Display for PreparationDiagnosticFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Child preparation diagnostic failed: {:?}", self.preparation)
    }
}
impl Error for PreparationDiagnosticFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
