// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select one canonical source independently of enumeration order and retain partial outputs.
use super::*;
impl RecoveryOperation {
    #[expect(clippy::result_large_err, reason = "Keep funded RecoveryOperation cause inline")]
    pub(super) fn recover_capture(
        &mut self,
        phase: RecoveryPhase,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let input = self.input.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let source = input.capture.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let repository = self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let inventory = repository.recovery_capture_inventory(context)?;
        let candidate = match phase {
            RecoveryPhase::Capture { after } => {
                let selected = next_journal_digest(
                    after,
                    inventory
                        .entries()
                        .filter(|entry| entry.class() == DiagnosticEntryClass::Journal)
                        .map(|entry| entry.name_digest()),
                );
                selected.and_then(|name| {
                    inventory.entries().find(|entry| {
                        entry.class() == DiagnosticEntryClass::Journal
                            && entry.name_digest() == name
                    })
                })
            }
            RecoveryPhase::Producer { journal } => inventory.entries().find(|entry| {
                entry.class() == DiagnosticEntryClass::Journal && entry.name_digest() == journal
            }),
            _ => return Err(IncidentStoreError::RecoveryConflict),
        };
        let Some(candidate) = candidate else {
            if matches!(phase, RecoveryPhase::Producer { .. }) {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            self.next = Some(RecoveryPhase::Scan);
            return Ok(());
        };
        let digest = candidate.name_digest();
        self.origin = repository.discover_recovery_capture_origin(source, candidate, context)?;
        let Some(origin) = self.origin.as_ref() else {
            if matches!(phase, RecoveryPhase::Producer { .. }) {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            self.next = Some(RecoveryPhase::Capture { after: Some(digest) });
            return Ok(());
        };
        let subject = match phase {
            RecoveryPhase::Capture { .. } => IncidentSubject::Desktop(origin.origin().run()),
            RecoveryPhase::Producer { .. } => {
                sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(origin.origin())
                    .producer_subject()?
            }
            _ => return Err(IncidentStoreError::RecoveryConflict),
        };
        let output = self.historical.as_mut().ok_or(IncidentStoreError::Capacity)?;
        self.historical_state = Some(
            repository
                .recover_historical_capture(source, origin, subject, input.at, output, context)?,
        );
        self.next = Some(match phase {
            RecoveryPhase::Capture { .. } => RecoveryPhase::Producer { journal: digest },
            _ => RecoveryPhase::Capture { after: Some(digest) },
        });
        Ok(())
    }
    pub(super) fn rearm(&mut self, input: &mut Option<RecoveryInput>) -> Result<(), RecoveryIssue> {
        let proposed = input.as_ref().ok_or(RecoveryIssue::Identity)?;
        let original = self.input.as_ref().ok_or(RecoveryIssue::Identity)?;
        let same_source = match (&original.capture, &proposed.capture) {
            (None, None) => true,
            (Some(old), Some(new)) => old.same_source(new),
            _ => false,
        };
        if !self.retired
            || self.failed()
            || self.next != Some(proposed.phase)
            || proposed.scope != original.scope
            || proposed.at != original.at
            || !same_source
            || proposed.storage_operation.owner() != proposed.scope.operation().owner()
            || proposed.storage_operation == original.storage_operation
            || (proposed.phase != RecoveryPhase::Scan
                && proposed.storage_operation.id() <= original.storage_operation.id())
            || (proposed.phase == RecoveryPhase::Scan
                && proposed.storage_operation != proposed.scope.operation())
        {
            return Err(RecoveryIssue::Identity);
        }
        if let Some(owner) = &mut self.historical {
            owner.reset().map_err(|_| RecoveryIssue::Busy)?;
        }
        self.repository = None;
        self.pending = None;
        self.migration = None;
        self.origin = None;
        self.original = None;
        self.next = None;
        self.retired = false;
        self.historical_state = None;
        self.promotion_replayed = false;
        self.input = input.take();
        Ok(())
    }
}

fn next_journal_digest(
    after: Option<[u8; 32]>,
    names: impl Iterator<Item = [u8; 32]>,
) -> Option<[u8; 32]> {
    names.filter(|name| after.is_none_or(|after| *name > after)).min()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_cursor_uses_canonical_names_without_order_or_insert_index_assumptions() {
        let a = [1; 32];
        let b = [2; 32];
        let c = [3; 32];
        for values in [[a, b, c], [a, c, b], [b, a, c], [b, c, a], [c, a, b], [c, b, a]] {
            assert_eq!(next_journal_digest(None, values.into_iter()), Some(a));
            assert_eq!(next_journal_digest(Some(a), values.into_iter()), Some(b));
            assert_eq!(next_journal_digest(Some(b), values.into_iter()), Some(c));
            assert_eq!(next_journal_digest(Some(c), values.into_iter()), None);
        }
        assert_eq!(next_journal_digest(Some(b), [a, c, [0; 32]].into_iter()), Some(c));
        assert_eq!(next_journal_digest(Some(b), [a, b].into_iter()), None);
    }
}
