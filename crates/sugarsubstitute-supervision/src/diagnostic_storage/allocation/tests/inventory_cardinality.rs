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
