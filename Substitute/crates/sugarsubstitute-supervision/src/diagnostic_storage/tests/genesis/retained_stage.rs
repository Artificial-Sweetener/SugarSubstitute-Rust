// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deny genesis when an earlier ordinary native stage retains the fresh namespace.
use super::*;

fn retained_ordinary_stage(partial: bool) -> TestResult {
    admitted(move |context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .partial_stage = partial;
        let result = storage.stage(DiagnosticObject::Summary(incident()?), 6, &operation, &context);
        let mut ordinary = if partial {
            assert_eq!(
                result.err().ok_or("partial ordinary creation succeeded")?.code(),
                Code::Write
            );
            None
        } else {
            let mut stage = result?;
            stage.write_chunk(b"before", &context)?;
            stage.flush(&context)?;
            Some(stage)
        };
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        let original = storage.snapshot();
        assert_eq!(original.stages(), 1);
        assert_eq!(original.live_handles(), 1);
        assert!(context.memory().claim(1).is_err());
        assert_eq!(
            storage
                .stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context)
                .err()
                .ok_or("genesis issued beside an ordinary retained stage")?
                .code(),
            Code::InvalidIdentity
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        assert_eq!(storage.snapshot().stages(), original.stages());
        assert_eq!(storage.snapshot().live_handles(), original.live_handles());
        if let Some(stage) = &mut ordinary {
            assert_eq!(stage.flush(&context)?.read_facts().sha256(), digest(b"before"));
        }
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert!(storage.retire(&operation, &context).is_err());
        assert_eq!(storage.snapshot().live_handles(), 1);
        assert!(context.memory().claim(1).is_err());
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        storage.retire(&operation, &context)?;
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(ordinary);
        drop(proof);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn genesis_rejects_retained_ordinary_complete_stage_without_another_effect() -> TestResult {
    retained_ordinary_stage(false)
}
#[test]
fn genesis_rejects_retained_ordinary_partial_creation_without_another_effect() -> TestResult {
    retained_ordinary_stage(true)
}
