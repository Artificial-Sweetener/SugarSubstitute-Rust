// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read only a verified committed host-failure appendix under repository custody.
use super::{CommittedIncident, HostFailureFact, IncidentRepository, IncidentStoreError};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;

impl IncidentRepository {
    /// Read version-one host facts from the exact immutable Fault attachment generation.
    /// Legacy fault text remains preserved without being interpreted as structured host facts.
    /// # Errors
    /// Retains native read custody on failure; rejects changed bytes, identity, or malformed
    /// recognized host-failure data. All file work stays on the finite storage worker.
    pub fn read_host_failure(
        &mut self,
        current: &CommittedIncident,
        context: &JobContext,
    ) -> Result<Option<HostFailureFact>, IncidentStoreError> {
        let key = current.summary().seed().key();
        self.namespace(key)?;
        let PayloadDisposition::Retained(inventory) = current.summary().facts().payload() else {
            return Ok(None);
        };
        let Some(fact) = inventory.entries().find(|fact| fact.kind() == AttachmentKind::Fault)
        else {
            return Ok(None);
        };
        if fact.retained_bytes() > HostFailureFact::MAX_BYTES as u64 {
            return Ok(None);
        }
        let revision = current.payload_revision().ok_or(IncidentStoreError::MissingPayload)?;
        if self.reads.len() >= DIAGNOSTIC_READ_BOUND {
            return Err(IncidentStoreError::Capacity);
        }
        let index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Payload(key, revision, AttachmentKind::Fault),
            HostFailureFact::MAX_BYTES as u64,
            &self.operation,
            context,
        )?)?;
        let actual = self.reads.get_mut(index)?.facts();
        if actual.bytes() != fact.retained_bytes() || actual.sha256() != *fact.sha256() {
            return Err(IncidentStoreError::WrongDigest);
        }
        let source = self.reads.get_mut(index)?.whole_source(context)?;
        let result = if source.as_bytes().starts_with(b"host_failure_") {
            Some(HostFailureFact::decode(source.as_bytes())?)
        } else {
            None
        };
        self.close_read(index, context)?;
        Ok(result)
    }
}
