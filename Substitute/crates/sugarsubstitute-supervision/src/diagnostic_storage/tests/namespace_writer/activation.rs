// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish and reopen actual diagnostic files under retained Entry and writer custody.
use super::*;
use sha2::{Digest, Sha256};
fn publish(explicit: bool) -> Result<(), Box<dyn std::error::Error>> {
    run(explicit, PinFault::None, 0, |namespace, context| {
        let mut storage =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let key = operation(&namespace, 1)?;
        let operation = storage.begin_operation(key, &context)?;
        let request = namespace.diagnostic_write_request(key)?;
        let opening = storage.resolve_namespace(&request, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut genesis = match opening {
            DiagnosticStoreOpening::Created(genesis) => genesis,
            _ => return Err("Native genesis directory was not created".into()),
        };
        let publication = (|| -> TestResult {
            let bytes = b"ledger";
            let hash: [u8; 32] = Sha256::digest(bytes).into();
            let mut stage = storage.stage_genesis(
                &mut genesis,
                hash,
                bytes.len() as u64,
                &operation,
                &context,
            )?;
            stage.write_chunk(bytes, &context)?;
            let flushed = stage.flush(&context)?.read_facts();
            let activation = storage.activate(
                &mut stage,
                DiagnosticPredecessor::Absent,
                &operation,
                &context,
            )?;
            assert_eq!(activation.facts(), flushed);
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
            storage.inventory(&operation, &context)?;
            let mut successor =
                storage.stage(DiagnosticObject::StoreState, 7, &operation, &context)?;
            successor.write_chunk(b"updated", &context)?;
            let successor_facts = successor.flush(&context)?.read_facts();
            let replaced = storage.activate(
                &mut successor,
                DiagnosticPredecessor::Exact {
                    file_identity: flushed.file_identity(),
                    sha256: flushed.sha256(),
                },
                &operation,
                &context,
            )?;
            assert_eq!(replaced.facts(), successor_facts);
            let mut read = storage.read(DiagnosticObject::StoreState, 7, &operation, &context)?;
            let mut output = [0; 7];
            assert_eq!(read.read_chunk(0, &mut output, &context)?, 7);
            assert_eq!(&output, b"updated");
            read.retire(&context)?;
            Ok(())
        })();
        storage.retire(&operation, &context)?;
        publication.map_err(|cause| {
            let native = std::error::Error::source(cause.as_ref())
                .and_then(|source| source.downcast_ref::<crate::ClaimError>());
            format!(
                "Native publication: {cause}; stage={:?}; win32={:?}",
                native.map(crate::ClaimError::operation),
                native.and_then(crate::ClaimError::win32_code)
            )
            .into()
        })
    })
}
#[test]
fn native_account_genesis_stage_activates_under_existing_directory_pins()
-> Result<(), Box<dyn std::error::Error>> {
    publish(false)
}
#[test]
fn native_explicit_genesis_stage_activates_under_existing_directory_pins()
-> Result<(), Box<dyn std::error::Error>> {
    publish(true)
}
