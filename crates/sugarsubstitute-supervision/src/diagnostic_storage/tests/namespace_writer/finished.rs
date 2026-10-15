// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual Entry, native inventory and generation pins before finished-role admission.
mod publication;
use super::*;
use crate::diagnostic_storage::fixture::ProtectedCaptureSourceFixture;
use sha2::{Digest, Sha256};
use std::error::Error;

#[test]
fn native_initialized_store_prepares_finished_roles_with_original_residual()
-> Result<(), Box<dyn Error>> {
    run(
        true,
        PinFault::None,
        ProtectedCaptureSourceFixture::required_reservation_bytes(),
        |namespace, context| {
            let key = operation(&namespace, 1)?;
            let source_path = std::env::temp_dir().join(format!(
                "diagnostic-finished-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut source =
                ProtectedCaptureSourceFixture::create(&context, key.owner(), source_path.clone())?;
            let mut storage =
                DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
            let operation = storage.begin_operation(key, &context)?;
            let request = namespace.diagnostic_write_request(key)?;
            let opening = storage.resolve_namespace(&request, &operation, &context)?;
            storage.acquire_writer(&operation, &context)?;
            drop(storage.inventory(&operation, &context)?);
            let mut genesis = match opening {
                DiagnosticStoreOpening::Created(genesis) => genesis,
                _ => return Err("Actual fresh namespace did not create genesis".into()),
            };
            // Storage owns activation; semantic decoding is outside this allocation-owner fixture.
            let bytes = b"ledger";
            let mut stage = storage.stage_genesis(
                &mut genesis,
                Sha256::digest(bytes).into(),
                bytes.len() as u64,
                &operation,
                &context,
            )?;
            stage.write_chunk(bytes, &context)?;
            stage.flush(&context)?;
            storage.activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)?;
            drop(stage);
            drop(storage.inventory(&operation, &context)?);
            let mut read = storage.read(
                DiagnosticObject::StoreState,
                bytes.len() as u64,
                &operation,
                &context,
            )?;
            let mut output = [0; 6];
            assert_eq!(read.read_chunk(0, &mut output, &context)?, bytes.len());
            assert_eq!(&output, bytes);
            read.retire(&context)?;
            drop(read);
            let mut generation = None;
            let result = (|| -> TestResult {
                storage.prepare_capture_read(
                    source.root_mut(),
                    &mut generation,
                    &operation,
                    &context,
                )?;
                let before = storage
                    .shared
                    ._allocation
                    .fixture_residual_bytes()
                    .ok_or("Actual ledger absent")?;
                let scan = crate::protected_storage::record_file::StreamingPort::diagnostic_scan_state_layout()?;
                let prepared = storage.reserve_finished_owner(
                    generation.as_mut().ok_or("Actual read lease absent")?,
                    &operation,
                    &context,
                );
                let (cpp, rust) = prepared.map_err(|cause| -> TestError {
                let native = Error::source(&cause).and_then(|value| value.downcast_ref::<crate::ClaimError>());
                let inner = native.and_then(Error::source).and_then(|value| value.downcast_ref::<DiagnosticNativeAllocationFailure>());
                format!("finished-role admission outer={:?} inner={:?} post={:?} residual_before={before} residual_limit={} cpp={} rust={} native_scan_retained={} native_scan_peak={}",
                    cause.code(), inner.map(DiagnosticNativeAllocationFailure::code), inner.and_then(DiagnosticNativeAllocationFailure::post_control_failure),
                    DIAGNOSTIC_RESIDUAL_BYTES, FINISHED_OWNER_STORAGE_BYTES, FINISHED_RUST_STORAGE_BYTES, scan.retained_bytes(), scan.peak_bytes()).into()
            })?;
                assert_eq!(
                    storage.shared._allocation.fixture_residual_bytes(),
                    Some(before + FINISHED_OWNER_STORAGE_BYTES + FINISHED_RUST_STORAGE_BYTES)
                );
                assert!(
                    cpp.matches_generation(generation.as_ref().ok_or("Actual read lease absent")?)
                );
                assert!(rust.matches_cpp(&cpp));
                let rescanned = storage.inventory(&operation, &context).map_err(|cause| -> TestError {
                    let native = Error::source(&cause).and_then(|value| value.downcast_ref::<crate::ClaimError>());
                    let inner = native.and_then(Error::source).and_then(|value| value.downcast_ref::<DiagnosticNativeAllocationFailure>());
                    format!("held-finished inventory outer={:?} inner={:?} post={:?} actual_residual={:?} residual_limit={}",
                        cause.code(), inner.map(DiagnosticNativeAllocationFailure::code), inner.and_then(DiagnosticNativeAllocationFailure::post_control_failure),
                        storage.shared._allocation.fixture_residual_bytes(), DIAGNOSTIC_RESIDUAL_BYTES).into()
                })?;
                assert_eq!(rescanned.status(), DiagnosticInventoryStatus::Complete);
                let candidate = rescanned
                    .entries()
                    .find(|entry| entry.class() == DiagnosticEntryClass::StoreState)
                    .ok_or("Actual initialized state absent")?;
                let residual = storage.shared._allocation.fixture_residual_bytes();
                let mut held_read = storage.read_inventory(candidate, 65_536, &operation, &context)
                    .map_err(|cause| -> TestError {
                        let native = Error::source(&cause).and_then(|value| value.downcast_ref::<crate::ClaimError>());
                        let inner = native.and_then(Error::source).and_then(|value| value.downcast_ref::<DiagnosticNativeAllocationFailure>());
                        format!("held-finished publication read outer={:?} inner={:?} post={:?} residual={residual:?} limit={}",
                            cause.code(), inner.map(DiagnosticNativeAllocationFailure::code),
                            inner.and_then(DiagnosticNativeAllocationFailure::post_control_failure), DIAGNOSTIC_RESIDUAL_BYTES).into()
                    })?;
                let image = held_read.whole_source(&context)?;
                assert_eq!(image.as_bytes(), bytes);
                drop(image);
                held_read.retire(&context)?;
                drop(held_read);
                drop(rescanned);
                drop(cpp);
                drop(rust);
                assert_eq!(storage.shared._allocation.fixture_residual_bytes(), Some(before));
                let (cpp, rust) = storage.reserve_finished_owner(
                    generation.as_mut().ok_or("Actual read lease absent")?,
                    &operation,
                    &context,
                )?;
                let publication = publication::roundtrip(&mut storage, &operation, &context);
                drop(cpp);
                drop(rust);
                publication
            })();
            if let Some(generation) = generation.as_mut() {
                generation.retire(&context)?;
            }
            drop(generation);
            storage.retire(&operation, &context)?;
            source.retire(&context)?;
            drop(storage);
            drop(operation);
            drop(source);
            std::fs::remove_dir_all(source_path)?;
            result
        },
    )
}

#[test]
fn native_nonempty_stage_inventory_keeps_exact_recovery_and_rejects_foreign_identity()
-> Result<(), Box<dyn Error>> {
    run(true, PinFault::None, 0, |namespace, context| {
        let key = operation(&namespace, 1)?;
        let mut storage =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let operation = storage.begin_operation(key, &context)?;
        let request = namespace.diagnostic_write_request(key)?;
        let opening = storage.resolve_namespace(&request, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        drop(storage.inventory(&operation, &context)?);
        let mut genesis = match opening {
            DiagnosticStoreOpening::Created(genesis) => genesis,
            _ => return Err("Actual fresh namespace did not create genesis".into()),
        };
        let mut initial = storage.stage_genesis(
            &mut genesis,
            Sha256::digest(b"ledger").into(),
            6,
            &operation,
            &context,
        )?;
        initial.write_chunk(b"ledger", &context)?;
        initial.flush(&context)?;
        storage.activate(&mut initial, DiagnosticPredecessor::Absent, &operation, &context)?;
        drop(initial);
        drop(storage.inventory(&operation, &context)?);
        let mut stage = storage.stage(DiagnosticObject::StoreState, 6, &operation, &context)?;
        stage.write_chunk(b"staged", &context)?;
        let facts = stage.flush(&context)?.read_facts();
        stage.retire(&context)?;
        drop(stage);
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Complete);
        assert_eq!(
            inventory
                .entries()
                .filter(|entry| entry.class() == DiagnosticEntryClass::Stage)
                .count(),
            1
        );
        let expected =
            RecordedDiagnosticFileFacts::new(facts.file_identity(), facts.sha256(), facts.bytes())?;
        let foreign = RecordedDiagnosticFileFacts::new([9; 24], facts.sha256(), facts.bytes())?;
        let mut state = storage.check(&operation, &context)?;
        let rejected = state.call_native(|port| {
            port.recover_stage(DiagnosticObject::StoreState, key, foreign, &context)
        })?;
        assert!(rejected.is_err());
        let (id, actual) = state.call_native(|port| {
            port.recover_stage(DiagnosticObject::StoreState, key, expected, &context)
        })??;
        assert_eq!(actual, facts);
        state.call_native(|port| port.close(id, &context))??;
        drop(state);
        drop(inventory);
        drop(storage.inventory(&operation, &context)?);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
