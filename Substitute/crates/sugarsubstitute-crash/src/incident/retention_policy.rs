// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select bounded retention victims and observe checked reclamation independently of authority.
use super::{IncidentRepository, IncidentStoreError, StoredIncidentSummary};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::incident::*;
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Inert observations after exact payload groups and their intent journals settled.
pub struct RetentionSnapshot {
    reclaimed_bytes: u64,
    reclaimed_groups: usize,
    charged_bytes: u64,
    payload_groups: usize,
    summaries: usize,
    evictions: SummaryEvictions,
}
impl RetentionSnapshot {
    /// Observe bytes whose native closure and protected absence were checked.
    pub const fn reclaimed_bytes(&self) -> u64 {
        self.reclaimed_bytes
    }
    /// Observe fully settled payload groups; summaries are preserved.
    pub const fn reclaimed_payload_groups(&self) -> usize {
        self.reclaimed_groups
    }
    /// Observe actual remaining payload, staging and quarantine charges.
    pub const fn charged_bytes(&self) -> u64 {
        self.charged_bytes
    }
    /// Observe actual remaining physical payload generations.
    pub const fn payload_groups(&self) -> usize {
        self.payload_groups
    }
    /// Observe preserved summary slots after actual complete enumeration.
    pub const fn summaries(&self) -> usize {
        self.summaries
    }
    /// Observe the independently verified ledger; payload disposal does not increment it.
    pub const fn evictions(&self) -> SummaryEvictions {
        self.evictions
    }
}
impl IncidentRepository {
    /// Enforce current acknowledged age/count retention under the original finite operation.
    ///
    /// Each exact group finishes its real source/Journal retirement before selecting
    /// another group. The separate publication preflight must project incoming demand.
    /// Unknown or over-cap inventories remain preserved and cannot authorize reclamation.
    /// # Errors
    /// Regressing UTC, foreign/duplicate records, live pins, uncertain native effects
    /// or missing candidate/Journal capacity retain the original owners and causes.
    pub fn enforce_retention(
        &mut self,
        now: IncidentUtc,
        context: &JobContext,
    ) -> Result<RetentionSnapshot, IncidentStoreError> {
        if self.pending.is_some()
            || self.preflight.is_some()
            || !self.reads.is_empty()
            || !self.stages.is_empty()
            || self.retention_utc.is_some_and(|prior| now < prior)
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.retention_utc = Some(now);
        let evictions = self.ledger(context)?;
        let mut reclaimed_bytes = 0_u64;
        let mut reclaimed_groups = 0_usize;
        loop {
            let inventory = self.inventory(context)?;
            if inventory.entries().any(|entry| {
                matches!(
                    entry.class(),
                    DiagnosticEntryClass::Journal
                        | DiagnosticEntryClass::Stage
                        | DiagnosticEntryClass::Quarantine
                )
            }) {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            let custody = self.storage.recovery_collection(&self.operation, context)?;
            let mut records = super::allocation::SummaryCollection::new(custody)?;
            for entry in
                inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Summary)
            {
                if records.len() == MAX_SUMMARIES {
                    return Err(IncidentStoreError::Capacity);
                }
                let (bytes, facts, index) = self.read_candidate(entry, context)?;
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                let stored = super::allocation::decode(&mut decode, || {
                    super::codec_summary::decode_current_summary(bytes.as_bytes())
                })?;
                self.namespace(stored.summary().seed().key())?;
                self.reads.get_mut(index)?.bind_object(
                    DiagnosticObject::Summary(stored.summary().seed().key()),
                    context,
                )?;
                self.close_read(index, context)?;
                records.records.push((stored, facts));
            }
            let Some(victim) = select_payload(
                records.iter().map(|(stored, _)| stored),
                now,
                inventory.payload_bytes(),
                inventory.payload_groups(),
                0,
                0,
                None,
            )?
            else {
                return Ok(RetentionSnapshot {
                    reclaimed_bytes,
                    reclaimed_groups,
                    charged_bytes: inventory.payload_bytes(),
                    payload_groups: inventory.payload_groups(),
                    summaries: inventory.summary_slots(),
                    evictions,
                });
            };
            let selected = records
                .iter()
                .position(|(stored, _)| {
                    stored.summary().seed().key() == victim.key
                        && stored.summary().revision() == victim.revision
                })
                .ok_or(IncidentStoreError::RecoveryConflict)?;
            let (stored, facts) = records.transfer(selected, context)?;
            drop(inventory);
            let bytes = self.discard_retained_payload(stored, facts, victim.reason, context)?;
            reclaimed_bytes =
                reclaimed_bytes.checked_add(bytes).ok_or(IncidentStoreError::Capacity)?;
            reclaimed_groups =
                reclaimed_groups.checked_add(1).ok_or(IncidentStoreError::Capacity)?;
        }
    }
    pub(super) fn project_publication(
        &mut self,
        demand: &super::IncidentPublicationDemand,
        now: IncidentUtc,
        selected: IncidentKey,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        self.ledger(context)?;
        self.retention_utc = Some(now);
        let reserved = demand
            .payload_bytes()
            .checked_add(demand.staging_headroom())
            .ok_or(IncidentStoreError::Capacity)?;
        loop {
            let (inventory, records) = self.publication_inventory(now, context)?;
            if let Some(victim) = select_payload(
                records.iter().map(|(stored, _)| stored),
                now,
                inventory.payload_bytes(),
                inventory.payload_groups(),
                reserved,
                demand.payload_groups(),
                Some(selected),
            )? {
                let selected = records
                    .iter()
                    .position(|(stored, _)| {
                        stored.summary().seed().key() == victim.key
                            && stored.summary().revision() == victim.revision
                    })
                    .ok_or(IncidentStoreError::RecoveryConflict)?;
                let (stored, facts) = records.transfer(selected, context)?;
                drop(inventory);
                self.discard_retained_payload(stored, facts, victim.reason, context)?;
                continue;
            }
            if inventory
                .payload_bytes()
                .checked_add(reserved)
                .is_none_or(|sum| sum > MAX_PAYLOAD_BYTES)
                || inventory
                    .payload_groups()
                    .checked_add(demand.payload_groups())
                    .is_none_or(|sum| sum > MAX_FULL_INCIDENTS)
                || inventory
                    .summary_slots()
                    .checked_add(demand.summary_slots())
                    .is_none_or(|sum| sum > MAX_SUMMARIES)
            {
                return Err(IncidentStoreError::Capacity);
            }
            return Ok(());
        }
    }
}

/// One inert planned payload disposition; actual storage must independently retain its sources.
pub(super) struct PayloadVictim {
    pub(super) key: IncidentKey,
    pub(super) revision: IncidentRevision,
    pub(super) reason: PayloadDiscard,
}
/// Select one exact retained group using original UTC facts and deterministic full-key ties.
/// Unknown physical bytes remain in the supplied charged total; they cannot authorize deletion.
pub(super) fn select_payload<'a, I>(
    summaries: I,
    now: IncidentUtc,
    charged_bytes: u64,
    physical_groups: usize,
    reserved_bytes: u64,
    reserved_groups: usize,
    excluded: Option<IncidentKey>,
) -> Result<Option<PayloadVictim>, IncidentStoreError>
where
    I: Clone + ExactSizeIterator<Item = &'a StoredIncidentSummary>,
{
    if summaries.len() > MAX_SUMMARIES
        || charged_bytes > MAX_PAYLOAD_BYTES
        || physical_groups > MAX_FULL_INCIDENTS
    {
        return Err(IncidentStoreError::Capacity);
    }
    let pressure = charged_bytes.checked_add(reserved_bytes).ok_or(IncidentStoreError::Capacity)?
        > MAX_PAYLOAD_BYTES
        || physical_groups.checked_add(reserved_groups).ok_or(IncidentStoreError::Capacity)?
            > MAX_FULL_INCIDENTS;
    let mut acknowledged = 0_usize;
    for (index, stored) in summaries.clone().enumerate() {
        let summary = stored.summary();
        let retention = summary.facts().retention();
        if retention.occurred() > now
            || retention.acknowledged().is_some_and(|at| at > now)
            || summaries
                .clone()
                .take(index)
                .any(|prior| prior.summary().seed().key() == summary.seed().key())
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if matches!(summary.facts().payload(), PayloadDisposition::Retained(_))
            && retention.acknowledged().is_some()
        {
            acknowledged += 1;
        }
    }
    let mut selected: Option<(&StoredIncidentSummary, u8, u64, PayloadDiscard)> = None;
    for stored in summaries {
        let summary = stored.summary();
        if excluded == Some(summary.seed().key())
            || !matches!(summary.facts().payload(), PayloadDisposition::Retained(_))
        {
            continue;
        }
        let retention = summary.facts().retention();
        let (rank, at, reason) = if let Some(acknowledged_at) = retention.acknowledged() {
            let expired = now.unix_seconds() - acknowledged_at.unix_seconds()
                >= ACKNOWLEDGED_RETENTION_SECONDS;
            let excess = acknowledged > MAX_ACKNOWLEDGED_FULL_INCIDENTS;
            if expired || excess {
                (0, acknowledged_at.unix_seconds(), PayloadDiscard::AcknowledgedRetention)
            } else if pressure {
                (1, acknowledged_at.unix_seconds(), PayloadDiscard::StoragePressure)
            } else {
                continue;
            }
        } else if pressure {
            (2, retention.occurred().unix_seconds(), PayloadDiscard::StoragePressure)
        } else {
            continue;
        };
        let order = (rank, at, summary.seed().key());
        if selected.as_ref().is_none_or(|(prior, prior_rank, prior_at, _)| {
            order < (*prior_rank, *prior_at, prior.summary().seed().key())
        }) {
            selected = Some((stored, rank, at, reason));
        }
    }
    Ok(selected.map(|(stored, _, _, reason)| PayloadVictim {
        key: stored.summary().seed().key(),
        revision: stored.summary().revision(),
        reason,
    }))
}
