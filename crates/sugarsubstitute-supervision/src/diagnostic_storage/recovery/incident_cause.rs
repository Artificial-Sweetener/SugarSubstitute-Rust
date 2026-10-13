// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry the Incident owner's concrete cause with its original outside control loan.
use crate::diagnostic_storage::DiagnosticIncidentValidationCauseLoan;
use std::{error::Error, sync::Arc};

/// Own one actual Incident cause allocation without exposing its control or claim.
/// Construction is restricted to the authored Incident error producer by governance.
pub struct DiagnosticIncidentValidationCause {
    source: Option<Arc<dyn Error + Send + Sync>>,
    allocation: DiagnosticIncidentValidationCauseLoan,
}
impl DiagnosticIncidentValidationCause {
    /// Move an already-admitted fresh Incident source and its original loan together.
    /// This allocation-free cross-crate transfer accepts no capacity, type or layout.
    /// The Incident producer proves its concrete payload against the fixed destination
    /// and transfers a fresh Arc without retaining a strong/weak projection.
    /// Authored-source governance permits this constructor only in that producer.
    pub fn from_incident_source(
        source: Arc<dyn Error + Send + Sync>,
        allocation: DiagnosticIncidentValidationCauseLoan,
    ) -> Self {
        Self { source: Some(source), allocation }
    }
    pub(super) fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as &(dyn Error + 'static))
    }
}
impl Clone for DiagnosticIncidentValidationCause {
    fn clone(&self) -> Self {
        Self { source: self.source.clone(), allocation: self.allocation.clone_alias() }
    }
}
impl Drop for DiagnosticIncidentValidationCause {
    fn drop(&mut self) {
        // The complete cause Arc/control frees before the outside credit alias drops.
        drop(self.source.take());
    }
}
