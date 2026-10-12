// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify recovery candidates retain actual custody without filename-derived subjects.
use super::*;
#[test]
fn diagnostic_seed_slots_and_full_payload_groups_reject_new_effects_at_literal_limits() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        {
            let mut disk = filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for index in 1..=128_u8 {
                disk.observations.push(crate::diagnostic_storage::inventory::ObservedEntry {
                    digest: [index; 32],
                    class: DiagnosticEntryClass::Seed,
                    identity: [index; 24],
                    bytes: 1,
                    protected_regular: true,
                    payload_group: None,
                });
            }
            for index in 1..=32_u8 {
                disk.observations.push(crate::diagnostic_storage::inventory::ObservedEntry {
                    digest: [index; 32],
                    class: DiagnosticEntryClass::Payload,
                    identity: [index; 24],
                    bytes: 1,
                    protected_regular: true,
                    payload_group: Some([index; 32]),
                });
            }
        }
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Complete);
        assert_eq!(inventory.seed_slots(), 128);
        assert_eq!(inventory.payload_groups(), 32);
        for object in [
            DiagnosticObject::Seed(incident()?),
            DiagnosticObject::Payload(incident()?, IncidentRevision::new(1)?, AttachmentKind::Log),
        ] {
            assert_eq!(
                storage
                    .stage(object, 1, &operation, &context)
                    .err()
                    .ok_or("literal cap bypassed")?
                    .code(),
                Code::Capacity
            );
        }
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_inventory_arithmetic_overflow_never_becomes_a_complete_empty_inventory() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        {
            let mut disk = filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for bytes in [u64::MAX, 1] {
                disk.observations.push(crate::diagnostic_storage::inventory::ObservedEntry {
                    digest: [4; 32],
                    class: DiagnosticEntryClass::Quarantine,
                    identity: [1; 24],
                    bytes,
                    protected_regular: true,
                    payload_group: None,
                });
            }
        }
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Incomplete);
        assert_eq!(inventory.payload_bytes(), u64::MAX);
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 1, &operation, &context)
                .err()
                .ok_or("overflow inventory admitted")?
                .code(),
            Code::InventoryIncomplete
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_unknown_and_saturated_scan_preserve_bytes_and_block_mutation() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
            crate::diagnostic_storage::inventory::ObservedEntry {
                digest: [4; 32],
                class: DiagnosticEntryClass::Unknown,
                identity: [0; 24],
                bytes: u64::MAX,
                protected_regular: false,
                payload_group: None,
            },
        );
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Incomplete);
        assert_eq!(inventory.unknown_entries(), 1);
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 64, &operation, &context)
                .err()
                .ok_or("unknown admitted mutation")?
                .code(),
            Code::InventoryIncomplete
        );
        let mut disk = filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        disk.observations.clear();
        disk.faults.scan_incomplete = true;
        drop(disk);
        assert_eq!(
            storage.inventory(&operation, &context)?.status(),
            DiagnosticInventoryStatus::Incomplete
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_inventory_read_revalidates_actual_identity_after_adopting_output() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        let facts = filesystem.put(object, b"candidate");
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
            crate::diagnostic_storage::inventory::ObservedEntry {
                digest: [4; 32],
                class: DiagnosticEntryClass::Summary,
                identity: facts.identity,
                bytes: facts.bytes,
                protected_regular: true,
                payload_group: None,
            },
        );
        let inventory = storage.inventory(&operation, &context)?;
        let entry = inventory.entries().next().ok_or("missing candidate")?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .changed_inventory = true;
        assert_eq!(
            storage
                .read_inventory(entry, 64, &operation, &context)
                .err()
                .ok_or("changed candidate admitted")?
                .code(),
            Code::Read
        );
        assert_eq!(storage.snapshot().live_handles(), 1);
        assert_eq!(storage.snapshot().reads(), 1);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_charged_stages_and_quarantine_never_admit_over_budget() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
            crate::diagnostic_storage::inventory::ObservedEntry {
                digest: [4; 32],
                class: DiagnosticEntryClass::Quarantine,
                identity: [1; 24],
                bytes: MAX_PAYLOAD_BYTES,
                protected_regular: true,
                payload_group: None,
            },
        );
        assert_eq!(storage.inventory(&operation, &context)?.payload_bytes(), MAX_PAYLOAD_BYTES);
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 1, &operation, &context)
                .err()
                .ok_or("overbudget stage admitted")?
                .code(),
            Code::Capacity
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_oversized_metadata_preserves_inventory_and_rejects_before_read_effects() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
            crate::diagnostic_storage::inventory::ObservedEntry {
                digest: [4; 32],
                class: DiagnosticEntryClass::Summary,
                identity: [1; 24],
                bytes: 65537,
                protected_regular: true,
                payload_group: None,
            },
        );
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Incomplete);
        let candidate = inventory.entries().next().ok_or("missing oversized observation")?;
        assert_eq!(candidate.bytes(), 65537);
        assert_eq!(
            storage
                .read_inventory(candidate, 65536, &operation, &context)
                .err()
                .ok_or("oversized read admitted")?
                .code(),
            Code::InvalidIdentity
        );
        assert_eq!(storage.snapshot().live_handles(), 0);
        assert_eq!(storage.snapshot().reads(), 0);
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 1, &operation, &context)
                .err()
                .ok_or("oversized metadata admitted mutation")?
                .code(),
            Code::InventoryIncomplete
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
