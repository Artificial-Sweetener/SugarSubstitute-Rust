// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine entry endpoint, overlapped I/O and actual kernel identity inspection to Windows.
#![allow(unsafe_code)]
mod native;
mod security;

use crate::{
    CoordinatorEntryClientInput, CoordinatorEntryClientSession, CoordinatorEntryServerInput,
    CoordinatorEntryServerSession, EntryTransportAdmissionError, ExecutionClient,
};
use sugarsubstitute_contracts::execution::{JobSpec, Moment, Scope};
impl ExecutionClient {
    /// Reserve one persistent entry server before constructing its derived native endpoint.
    /// Finite preparation is separate from the required no-deadline native scope and ancestors.
    /// # Errors
    /// Returns actual reservation, original preparation, or inert shape failure before native effects.
    pub fn reserve_coordinator_entry_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: CoordinatorEntryServerInput,
        original_preparation: Moment,
    ) -> Result<CoordinatorEntryServerSession, EntryTransportAdmissionError> {
        let lease = Box::new(native::Lease::new(true, *input.facts()));
        crate::coordinator_entry::reserve_server(
            &self.core,
            scope,
            spec,
            input,
            original_preparation,
            lease,
        )
    }
    /// Reserve one original request; retained staging permits bounded Initializing re-probes.
    /// The persistent scope, every ancestor and job must have no deadline.
    /// # Errors
    /// Returns actual reservation or conservative source counter conversion failure before native effects.
    pub fn reserve_coordinator_entry_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: CoordinatorEntryClientInput,
    ) -> Result<CoordinatorEntryClientSession, EntryTransportAdmissionError> {
        let lease = Box::new(native::Lease::new(false, input.facts));
        crate::coordinator_entry::reserve_client(&self.core, scope, spec, input, lease)
    }
}
