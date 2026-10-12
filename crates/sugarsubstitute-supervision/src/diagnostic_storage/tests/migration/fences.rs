// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence callback reentry and revalidate actual external source changes before effects.
use super::*;
#[test]
fn diagnostic_migration_callback_reentry_is_fenced_outside_owner_mutex() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator, original) =
            normal_with_read(&mut storage, &operation, &context)?;
        let original = Arc::new(Mutex::new(original));
        let reentry_original = original.clone();
        let reentry_context = context.clone();
        *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Box::new(move || {
                let mut original =
                    reentry_original.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                assert_eq!(
                    original.retire(&reentry_context).err().map(|error| error.code()),
                    Some(Code::Busy)
                );
            }));
        let candidate =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        storage.finish_summary_migration(
            &mut permit,
            &candidate,
            ARCHIVE,
            &validator,
            &operation,
            &context,
        )?;
        storage.retire(&operation, &context)?;
        let weak = Arc::downgrade(&storage.shared);
        drop((storage, permit, candidate, operation, validator, original));
        assert!(
            weak.upgrade().is_none(),
            "retired first-party state retained after every live facade is dropped"
        );
        Ok(())
    })
}
#[test]
fn diagnostic_migration_callback_source_tampering_blocks_native_candidate_effect() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        let changed = filesystem.clone();
        *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Box::new(move || changed.change_stage(1)));
        let mutations = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .complete_summary_migration(&mut permit, &validator, &operation, &context)
                .err()
                .ok_or("changed source published")?
                .code(),
            Code::ChangedIdentity
        );
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert!(storage.snapshot().disposition_retained());
        assert_eq!(storage.snapshot().live_handles(), 4);
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        Ok(())
    })
}
