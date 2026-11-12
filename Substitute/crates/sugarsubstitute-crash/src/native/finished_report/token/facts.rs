// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow parser-confirmed data from the same retained token without exporting authority.
use super::*;
impl Token {
    pub(crate) fn failure_diagnostic(
        &self,
    ) -> Option<super::super::diagnostic::FinishedFailureDiagnostic> {
        self.failure().map(Failure::diagnostic)
    }
    pub(crate) fn publication_headroom(&self) -> Option<u64> {
        let prepared = self.fields.as_ref()?.state.preparation.as_ref()?;
        [
            prepared.source.bytes,
            prepared.metadata.bytes,
            prepared.original_stage.file.bytes,
            prepared.candidate_stage.file.bytes,
        ]
        .into_iter()
        .try_fold(131072u64, u64::checked_add)
    }

    pub(crate) fn panic_payload(&self) -> Option<&(dyn std::any::Any + Send)> {
        match self.failure()? {
            Failure::Control(cause) => cause.panic_payload(),
            _ => None,
        }
    }
    pub(crate) fn cause(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.failure().map(|value| value as _)
    }
    pub(crate) fn dump(&self) -> Option<(DumpId, u64)> {
        let fields = self.fields.as_ref()?;
        if fields.phase != Phase::Issued || fields.state.token == 0 {
            return None;
        }
        let facts = fields.state.facts.as_ref()?;
        Some((DumpId::new(facts.uuid).ok()?, facts.source_bytes))
    }
    pub(crate) fn matches(
        &self,
        incident: IncidentKey,
        build: &BuildIdentity,
        operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
    ) -> bool {
        self.fields.as_ref().is_some_and(|fields| {
            fields.operation == operation
                && fields.phase == Phase::Issued
                && fields.state.facts.as_ref().is_some_and(|facts| {
                    facts.subject == calls::subject(incident)
                        && facts.build_length as usize == build.as_str().len()
                        && facts.build.get(..facts.build_length as usize)
                            == Some(build.as_str().as_bytes())
                })
        })
    }
}
