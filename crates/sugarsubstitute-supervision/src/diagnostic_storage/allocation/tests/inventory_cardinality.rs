// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify closed cardinality charges, rejection and final original-claim retention.
use super::*;
use crate::diagnostic_storage::inventory::ObservedEntry;
use std::mem::size_of;

#[test]
fn counted_inventory_roles_reject_over_bound_and_retain_exact_cells_until_drop() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let owner = storage.shared._allocation.clone();
        let before = owner.fixture_residual_bytes().ok_or("Original ledger absent")?;
        let binding = allocation::scan_binding(&owner, operation.key(), 2)?;
        let binding_bytes =
            2 * size_of::<ObservedEntry>() + DIAGNOSTIC_SCAN_BOUND * 3 * size_of::<[u8; 32]>();
        assert_eq!(owner.fixture_residual_bytes(), Some(before + binding_bytes));
        let inventory = allocation::inventory(&owner, operation.key(), 2)?;
        let inventory_bytes =
            size_of::<DiagnosticInventory>() + 2 * size_of::<DiagnosticInventoryEntry>();
        assert_eq!(owner.fixture_residual_bytes(), Some(before + binding_bytes + inventory_bytes));
        for cause in [
            allocation::inventory(&owner, operation.key(), DIAGNOSTIC_SCAN_BOUND + 1)
                .err()
                .ok_or("Oversized inventory admitted")?,
            allocation::scan_binding(&owner, operation.key(), DIAGNOSTIC_SCAN_BOUND + 1)
                .err()
                .ok_or("Oversized binding admitted")?,
        ] {
            assert_eq!(cause.code(), Code::Capacity);
        }
        assert_eq!(owner.fixture_residual_bytes(), Some(before + binding_bytes + inventory_bytes));
        storage.retire(&operation, &context)?;
        drop(storage);
        drop(operation);
        drop(owner);
        assert!(context.memory().claim(1).is_err());
        drop(binding);
        assert!(context.memory().claim(1).is_err());
        drop(inventory);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn inventory_admits_actual_cardinality_and_rejects_unavailable_simultaneous_destinations()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let owner = storage.shared._allocation.clone();
        let binding_base = DIAGNOSTIC_SCAN_BOUND * 3 * size_of::<[u8; 32]>();
        let binding_max = binding_base + DIAGNOSTIC_SCAN_BOUND * size_of::<ObservedEntry>();
        let inventory_max = size_of::<DiagnosticInventory>()
            + DIAGNOSTIC_SCAN_BOUND * size_of::<DiagnosticInventoryEntry>();
        let mut buffers = Vec::new();
        while owner.fixture_residual_bytes().ok_or("Original ledger absent")? - binding_base
            + binding_max
            + inventory_max
            <= DIAGNOSTIC_RESIDUAL_BYTES
        {
            buffers.push(storage.record_buffer(&operation, &context)?);
        }
        let before = owner.fixture_residual_bytes().ok_or("Original ledger absent")?;
        let baseline = before - binding_base;
        assert!(baseline + binding_max <= DIAGNOSTIC_RESIDUAL_BYTES);
        assert!(baseline + inventory_max <= DIAGNOSTIC_RESIDUAL_BYTES);
        assert!(baseline + binding_max + inventory_max > DIAGNOSTIC_RESIDUAL_BYTES);
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Complete);
        assert_eq!(inventory.entries().count(), 0);
        assert_eq!(owner.fixture_residual_bytes(), Some(before + size_of::<DiagnosticInventory>()));
        drop(inventory);
        assert_eq!(owner.fixture_residual_bytes(), Some(before));
        {
            let mut disk = filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for index in 0..DIAGNOSTIC_SCAN_BOUND {
                let mut digest = [1; 32];
                digest[..8].copy_from_slice(&(index as u64).to_le_bytes());
                let mut identity = [1; 24];
                identity[..8].copy_from_slice(&(index as u64).to_le_bytes());
                disk.observations.push(ObservedEntry {
                    digest,
                    class: DiagnosticEntryClass::Journal,
                    identity,
                    bytes: 1,
                    protected_regular: true,
                    payload_group: None,
                });
            }
        }
        let mutations = filesystem.snapshot().mutation_count();
        let generation = lock(&storage.shared).scan_generation;
        let rejected = storage
            .inventory(&operation, &context)
            .err()
            .ok_or("Unavailable actual simultaneous destinations admitted")?;
        assert_eq!(rejected.code(), Code::Capacity);
        assert_eq!(lock(&storage.shared).scan_generation, generation + 1);
        assert_eq!(owner.fixture_residual_bytes(), Some(baseline));
        assert_eq!(
            filesystem
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .observations
                .len(),
            DIAGNOSTIC_SCAN_BOUND
        );
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(storage.snapshot().live_handles(), 0);
        let rejected = storage
            .stage(DiagnosticObject::StoreState, 1, &operation, &context)
            .err()
            .ok_or("Failed inventory authorized mutation")?;
        assert_eq!(rejected.code(), Code::InventoryIncomplete);
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        storage.retire(&operation, &context)?;
        drop(storage);
        drop(operation);
        drop(owner);
        assert!(context.memory().claim(1).is_err());
        drop(buffers);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
