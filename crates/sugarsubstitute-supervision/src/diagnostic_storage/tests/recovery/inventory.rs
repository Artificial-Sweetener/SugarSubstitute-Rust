// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind recovered journals and ledgers to actual canonical inventory sources.
use super::*;
use crate::diagnostic_storage::inventory::ObservedEntry;
fn observe(filesystem: &Filesystem, object: DiagnosticObject, bytes: &[u8]) -> TestResult {
    let facts = filesystem.put(object, bytes);
    let (class, digest) = crate::protected_storage::record_file::diagnostic_object_binding(
        object,
        owner_key(2)?.namespace,
    );
    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
        ObservedEntry {
            digest,
            class,
            identity: facts.identity,
            bytes: facts.bytes,
            protected_regular: true,
            payload_group: None,
        },
    );
    Ok(())
}
#[test]
fn diagnostic_recovery_inventory_binds_full_historical_key_and_preserves_wrong_class_or_name()
-> TestResult {
    for fault in 0..5 {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let historical =
                LogicalStoreOperationKey::new(owner_key(3)?, IncidentOperationId::new(1)?);
            let plan = recovery_plan(historical)?;
            observe(&filesystem, DiagnosticObject::Journal(historical), JOURNAL)?;
            observe(&filesystem, DiagnosticObject::StoreState, ORIGINAL)?;
            if fault != 0 {
                let mut disk =
                    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                match fault {
                    1 => disk.observations[0].class = DiagnosticEntryClass::Summary,
                    2 => {
                        disk.observations[0].digest =
                            crate::protected_storage::record_file::diagnostic_object_binding(
                                DiagnosticObject::Journal(key(1)?),
                                owner_key(2)?.namespace,
                            )
                            .1
                    }
                    3 => disk.observations[1].class = DiagnosticEntryClass::Journal,
                    4 => disk.observations[1].digest = [7; 32],
                    _ => return Err("unknown fixture fault".into()),
                }
            }
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let inventory = storage.inventory(&operation, &context)?;
            let mut entries = inventory.entries();
            let mut journal = storage.read_inventory(
                entries.next().ok_or("journal missing")?,
                65536,
                &operation,
                &context,
            )?;
            let mut ledger = storage.read_inventory(
                entries.next().ok_or("ledger missing")?,
                65536,
                &operation,
                &context,
            )?;
            let result = storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &validator(plan, DiagnosticRecoveryLedgerCase::Original),
                &operation,
                &context,
            );
            if fault == 0 {
                let permit = result?;
                assert_eq!(permit.operation, operation.key());
                assert_eq!(storage.snapshot().live_handles(), 1);
                assert_eq!(storage.snapshot().reads(), 0);
                assert!(storage.snapshot().disposition_retained());
            } else {
                assert_eq!(
                    result.err().ok_or("foreign candidate accepted")?.code(),
                    Code::InvalidIdentity
                );
                assert_eq!(storage.snapshot().live_handles(), 2);
                assert_eq!(storage.snapshot().reads(), 2);
                assert!(!storage.snapshot().disposition_retained());
            }
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                0
            );
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
