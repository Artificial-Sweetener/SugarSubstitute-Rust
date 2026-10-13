// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt a uniquely observed historical stage without creating or rewriting bytes.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StageRecoveryAccess {
    Ledger,
    PreservedOriginal,
    Candidate,
}
impl StreamingPort {
    pub(super) fn recover_stage_owned(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        access: StageRecoveryAccess,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.check(context)?;
        if !self.stage_scan_complete
            || !self.writer.as_ref().is_some_and(HeldFile::is_locked)
            || !match access {
                StageRecoveryAccess::Ledger => object == DiagnosticObject::StoreState,
                _ => matches!(object, DiagnosticObject::Summary(_)),
            }
            || Some(historical.owner().namespace) != self.namespace
            || expected.bytes() > DIAGNOSTIC_IO_BYTES as u64
        {
            return Err(self.reject());
        }
        let namespace = self.namespace.ok_or_else(|| self.reject())?;
        let prefix = leaf::Leaf::recovery_prefix(object, namespace, historical)
            .map_err(|_| self.reject())?;
        let prefix = prefix.text().map_err(|_| self.reject())?;
        let scan = self.stage_candidates.as_ref().ok_or_else(|| self.reject())?;
        let mut selected = None;
        for (index, candidate) in scan.candidates.iter().enumerate() {
            if candidate.identity == expected.file_identity()
                && candidate.bytes == expected.bytes()
                && candidate.leaf.starts_with(prefix)
            {
                if selected.is_some() {
                    return Err(self.reject());
                }
                selected = Some(index);
            }
        }
        let candidate = &scan.candidates[selected.ok_or_else(|| self.reject())?];
        let path = self.name(&candidate.leaf, context)?;
        let writable = access != StageRecoveryAccess::PreservedOriginal;
        let (id, actual) = self.open_file(path, expected.bytes(), false, writable, context)?;
        let index = self.index(id)?;
        if expected.file_identity() != actual.identity
            || expected.sha256() != actual.sha256
            || expected.bytes() != actual.bytes
        {
            return Err(self.invalid("diagnostic_changed_recovery_stage"));
        }
        self.files[index].fields_mut()?.stage = writable;
        self.check(context)?;
        Ok((id, actual))
    }
}
