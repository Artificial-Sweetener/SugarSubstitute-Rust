// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate the bounded registry table only inside its original admitted job.
use super::*;
use sugarsubstitute_contracts::execution::MemoryLease;

const TABLE_SLOTS: usize = 131;
const ORIGINAL_TABLE_BYTES: u64 = 370_120;
const _: () = assert!(WindowsProcessRegistry::storage_bytes() <= ORIGINAL_TABLE_BYTES);

impl WindowsProcessRegistry {
    /// Return the fixed original byte reservation required for registry construction.
    /// This covers the actual table/header, lifetime identity, native port, and all
    /// bounded pending Shared allocations before any creation factory is admitted.
    /// Each contained family's separate 131072-byte reservation remains unchanged.
    pub const fn required_reservation_bytes() -> u64 {
        ORIGINAL_TABLE_BYTES
    }
    /// Measure every actual allocation inside the unchanged original table reservation.
    pub(super) const fn storage_bytes() -> u64 {
        (size_of::<Self>()
            + TABLE_SLOTS * size_of::<Option<Record>>()
            + TABLE_SLOTS * (size_of::<Shared>() + 2 * size_of::<usize>())
            + RegistryIdentity::allocation_bytes()
            + 2 * size_of::<usize>()) as u64
    }

    /// Admit finite construction without allocating the record table on the caller.
    /// Consume the actual Success completion before using its owned registry.
    /// Terminal acknowledgement keeps its original memory reservation retained by
    /// the table and every outstanding custody/identity pin. No native effect runs.
    ///
    /// # Errors
    /// Rejects another operation/class, replacement, incorrect fixed reservation,
    /// foreign/closed scopes, expired deadlines, or actual execution saturation.
    /// Worker allocation errors preserve their cause in the typed terminal result.
    pub fn submit_new(
        execution: &ExecutionClient,
        scope: &Scope,
        specification: JobSpec,
        owner: CoordinatorKey,
    ) -> Result<OwnedJob<Self, RegistryError>, RegistryAdmissionError> {
        validate_blocking(specification)?;
        if specification.reservation_bytes != Self::required_reservation_bytes() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        execution
            .submit_blocking(scope, specification, move |context| {
                Self::allocate(owner, context.memory())
            })
            .map_err(RegistryAdmissionError::Execution)
    }

    /// Claim the complete fixed retained allocation before creating its actual table.
    fn allocate(owner: CoordinatorKey, memory: &MemoryLease) -> Result<Self, RegistryError> {
        let claim = memory
            .claim(Self::required_reservation_bytes())
            .map_err(|cause| RegistryError::new("registry_table_memory", cause))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(TABLE_SLOTS)
            .map_err(|cause| RegistryError::new("registry_records_allocation", cause))?;
        if records.capacity() != TABLE_SLOTS {
            return Err(RegistryError::invalid("registry_records_capacity"));
        }
        records.resize_with(TABLE_SLOTS, || None);
        Ok(Self {
            owner,
            records,
            identity: RegistryIdentity::new(claim),
            next_process: 1,
            launch_open: true,
            cursor: 0,
            cleanup_watches: 0,
            port: native_process::production(),
        })
    }
}
