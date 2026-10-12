// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt a uniquely observed historical stage without creating or rewriting bytes.
use super::*;
impl StreamingPort {
    pub(super) fn recover_stage_owned(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.check(context)?;
        if !self.stage_scan_complete
            || !self.writer.as_ref().is_some_and(HeldFile::is_locked)
            || object != DiagnosticObject::StoreState
            || Some(historical.owner().namespace) != self.namespace
            || expected.bytes() > DIAGNOSTIC_IO_BYTES as u64
        {
            return Err(ClaimError::invalid("diagnostic_unbound_stage_recovery"));
        }
        let namespace =
            self.namespace.ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?;
        let prefix = format!(
            "{}.{}.",
            layout::leaf(object, namespace),
            super::super::hex(&layout::operation(historical))
        );
        let mut matching = self.stage_candidates.iter().filter(|candidate| {
            candidate.identity == expected.file_identity()
                && candidate.bytes == expected.bytes()
                && candidate.leaf.starts_with(&prefix)
        });
        let leaf = matching
            .next()
            .ok_or_else(|| ClaimError::invalid("diagnostic_missing_recovery_stage"))?
            .leaf
            .clone();
        if matching.next().is_some() {
            return Err(ClaimError::invalid("diagnostic_ambiguous_recovery_stage"));
        }
        // OPEN_EXISTING adopts its output into this owner before identity/security/hash checks.
        let path = self
            .directory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?
            .join(leaf);
        let (id, actual) = self.open_file(path, expected.bytes(), false, true, context)?;
        let index = self.index(id)?;
        if expected.file_identity() != actual.file_identity()
            || expected.sha256() != actual.sha256()
            || expected.bytes() != actual.bytes()
        {
            return Err(ClaimError::invalid("diagnostic_changed_recovery_stage"));
        }
        self.files[index].stage = true;
        self.check(context)?;
        Ok((id, actual))
    }
}
