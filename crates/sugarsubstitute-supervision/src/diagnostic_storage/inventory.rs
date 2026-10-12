// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded physical observations before semantic incident codec recovery.
use super::*;

/// Categorize bounded physical entries without inventing their semantic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticEntryClass {
    /// Canonically named seed candidate, awaiting codec validation.
    Seed,
    /// Canonically named summary candidate, awaiting codec validation.
    Summary,
    /// Canonically named namespace ledger candidate.
    StoreState,
    /// Canonically named payload candidate; never opened through metadata recovery.
    Payload,
    /// Full-operation journal candidate, awaiting codec validation.
    Journal,
    /// Retained physical stage whose bytes remain charged.
    Stage,
    /// Retained quarantine whose bytes remain charged.
    Quarantine,
    /// Foreign/noncanonical/uncertain entry that cannot authorize access or deletion.
    Unknown,
}
impl DiagnosticEntryClass {
    pub(crate) const fn suffix(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Summary => "summary",
            Self::StoreState => "state",
            Self::Payload => "payload",
            Self::Journal => "journal",
            Self::Stage => "stage",
            Self::Quarantine => "quarantine",
            Self::Unknown => "unknown",
        }
    }
    pub(crate) const fn bound(self) -> u64 {
        match self {
            Self::Seed => 4096,
            Self::Summary | Self::StoreState | Self::Journal => 65536,
            Self::Payload | Self::Stage | Self::Quarantine | Self::Unknown => 0,
        }
    }
}
#[derive(Clone)]
pub(crate) struct ObservedEntry {
    pub(crate) digest: [u8; 32],
    pub(crate) class: DiagnosticEntryClass,
    pub(crate) identity: [u8; 24],
    pub(crate) bytes: u64,
    pub(crate) protected_regular: bool,
    pub(crate) payload_group: Option<[u8; 32]>,
}
impl ObservedEntry {
    pub(crate) fn readable(&self) -> bool {
        self.protected_regular
            && self.identity != [0; 24]
            && self.digest != [0; 32]
            && self.class.bound() != 0
            && self.bytes <= self.class.bound()
    }
}
pub(crate) struct Scan {
    pub(crate) entries: Vec<ObservedEntry>,
    pub(crate) complete: bool,
}

/// A constructor-private observation tied to the actual retained protected directory.
/// Its digest/suffix identify only a candidate, never a decoded incident key.
pub struct DiagnosticInventoryEntry {
    pub(super) shared: Shared,
    pub(super) namespace: NamespaceId,
    pub(super) observed: ObservedEntry,
}
impl DiagnosticInventoryEntry {
    /// Observe the verified namespace of its retained native directory custody.
    pub const fn namespace(&self) -> NamespaceId {
        self.namespace
    }
    /// Observe the canonical filename digest; this grants no arbitrary-path access.
    pub const fn name_digest(&self) -> [u8; 32] {
        self.observed.digest
    }
    /// Observe the physical candidate class before codec validation.
    pub const fn class(&self) -> DiagnosticEntryClass {
        self.observed.class
    }
    /// Observe the actual physical file identity, rechecked before a recovery read.
    pub const fn file_identity(&self) -> [u8; 24] {
        self.observed.identity
    }
    /// Observe actual physical bytes, including preserved staging/quarantine.
    pub const fn bytes(&self) -> u64 {
        self.observed.bytes
    }
}

/// Distinguish bounded complete native enumeration from preserved uncertainty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticInventoryStatus {
    /// The native scan observed every physical entry within literal bounds.
    Complete,
    /// Foreign entries, overflow, saturation or native uncertainty blocks mutation.
    Incomplete,
}

/// Own at most4096 observed candidates and checked physical accounting facts.
pub struct DiagnosticInventory {
    entries: Vec<DiagnosticInventoryEntry>,
    status: DiagnosticInventoryStatus,
    payload_bytes: u64,
    seed_slots: usize,
    summary_slots: usize,
    unknown_entries: usize,
    payload_groups: usize,
}
impl DiagnosticInventory {
    /// Borrow immutable observations while their protected directory remains retained.
    pub fn entries(&self) -> impl Iterator<Item = &DiagnosticInventoryEntry> {
        self.entries.iter()
    }
    /// Observe actual scan completeness; it is not semantic codec validation.
    pub const fn status(&self) -> DiagnosticInventoryStatus {
        self.status
    }
    /// Observe physical payload/stage/quarantine bytes with checked arithmetic.
    /// Incomplete scans expose only a checked subtotal; it cannot authorize capacity.
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    /// Observe physical seed candidates, including corrupt/unvalidated records.
    pub const fn seed_slots(&self) -> usize {
        self.seed_slots
    }
    /// Observe physical summary candidates, including corrupt/unvalidated records.
    pub const fn summary_slots(&self) -> usize {
        self.summary_slots
    }
    /// Observe foreign/unverified entries that remain preserved.
    pub const fn unknown_entries(&self) -> usize {
        self.unknown_entries
    }
    /// Observe distinct canonical physical payload-group digests before codec validation.
    pub const fn payload_groups(&self) -> usize {
        self.payload_groups
    }
}
impl DiagnosticStorage {
    /// Enumerate a bounded physical inventory and retain unknown entries unchanged.
    /// # Errors
    /// Native enumeration/context failure preserves actual custody. An incomplete
    /// returned snapshot disables mutation rather than inventing a complete total.
    pub fn inventory(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticInventory, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if state.migration.is_some() {
            return Err(error(operation.key, Code::Busy));
        }
        state.inventory_complete = false;
        let namespace = state.namespace.ok_or_else(|| error(operation.key, Code::Closed))?;
        let scan = state
            .port
            .scan(context)
            .map_err(|cause| native(operation.key, Code::InventoryIncomplete, cause))?;
        if scan.entries.len() > DIAGNOSTIC_SCAN_BOUND {
            return Err(error(operation.key, Code::InventoryIncomplete));
        }
        state.scan_generation = state
            .scan_generation
            .checked_add(1)
            .ok_or_else(|| error(operation.key, Code::Capacity))?;
        state.scan_entries.clear();
        state.scan_entries.try_reserve_exact(scan.entries.len()).map_err(|cause| {
            native(operation.key, Code::Capacity, ClaimError::new("diagnostic_scan_binding", cause))
        })?;
        state.scan_entries.extend(scan.entries.iter().cloned());
        let mut result = DiagnosticInventory {
            entries: Vec::new(),
            status: DiagnosticInventoryStatus::Incomplete,
            payload_bytes: 0,
            seed_slots: 0,
            summary_slots: 0,
            unknown_entries: 0,
            payload_groups: 0,
        };
        result.entries.try_reserve_exact(scan.entries.len()).map_err(|cause| {
            native(
                operation.key,
                Code::Capacity,
                ClaimError::new("diagnostic_inventory_reservation", cause),
            )
        })?;
        let mut complete = scan.complete;
        let mut groups = Vec::new();
        let mut seeds = Vec::new();
        let mut summaries = Vec::new();
        seeds
            .try_reserve_exact(DIAGNOSTIC_SCAN_BOUND)
            .and_then(|()| summaries.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND))
            .map_err(|cause| {
                native(
                    operation.key,
                    Code::Capacity,
                    ClaimError::new("diagnostic_slot_reservation", cause),
                )
            })?;
        groups.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND).map_err(|cause| {
            native(
                operation.key,
                Code::Capacity,
                ClaimError::new("diagnostic_group_reservation", cause),
            )
        })?;
        for observed in scan.entries {
            if let Some(group) = observed.payload_group
                && !groups.contains(&group)
            {
                groups.push(group);
            }
            match observed.class {
                DiagnosticEntryClass::Seed => {
                    result.seed_slots += 1;
                    seeds.push(observed.digest);
                }
                DiagnosticEntryClass::Summary => {
                    result.summary_slots += 1;
                    summaries.push(observed.digest);
                }
                DiagnosticEntryClass::Payload
                | DiagnosticEntryClass::Stage
                | DiagnosticEntryClass::Quarantine => {
                    match result.payload_bytes.checked_add(observed.bytes) {
                        Some(total) => result.payload_bytes = total,
                        None => complete = false,
                    }
                }
                DiagnosticEntryClass::Unknown => result.unknown_entries += 1,
                DiagnosticEntryClass::StoreState | DiagnosticEntryClass::Journal => {}
            }
            if !observed.protected_regular
                || observed.identity == [0; 24]
                || observed.digest == [0; 32]
                || (observed.class.bound() != 0 && observed.bytes > observed.class.bound())
            {
                complete = false;
            }
            result.entries.push(DiagnosticInventoryEntry {
                shared: self.shared.clone(),
                namespace,
                observed,
            });
        }
        result.payload_groups = groups.len();
        complete &= result.unknown_entries == 0
            && result.seed_slots <= DIAGNOSTIC_SEED_BOUND
            && groups.len() <= sugarsubstitute_contracts::incident::MAX_FULL_INCIDENTS
            && result.summary_slots <= sugarsubstitute_contracts::incident::MAX_SUMMARIES
            && result.payload_bytes <= sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES;
        check(&mut state, operation.key, context, false)?;
        state.inventory_complete = complete;
        if complete {
            state.charged_bytes = result.payload_bytes;
            state.payload_groups = groups;
            state.seeds = seeds;
            state.summaries = summaries;
        }
        if complete {
            result.status = DiagnosticInventoryStatus::Complete;
        }
        Ok(result)
    }
}
