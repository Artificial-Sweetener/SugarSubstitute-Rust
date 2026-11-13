// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject changed retry requests and third target generations before further native effects.
use super::*;
#[test]
fn diagnostic_migration_changed_archive_retry_preserves_same_actual_stage_without_effects()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        let candidate =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(
            storage
                .finish_summary_migration(
                    &mut permit,
                    &candidate,
                    ARCHIVE,
                    &validator,
                    &operation,
                    &context
                )
                .is_err()
        );
        let mutations = filesystem.snapshot().mutation_count();
        let outputs = filesystem.snapshot().live_outputs();
        assert_eq!(
            storage
                .finish_summary_migration(
                    &mut permit,
                    &candidate,
                    b"changed archive",
                    &validator,
                    &operation,
                    &context
                )
                .err()
                .ok_or("changed retry admitted")?
                .code(),
            Code::ChangedIdentity
        );
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(filesystem.snapshot().live_outputs(), outputs);
        assert!(storage.snapshot().disposition_retained());
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        storage.finish_summary_migration(
            &mut permit,
            &candidate,
            ARCHIVE,
            &validator,
            &operation,
            &context,
        )?;
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_migration_third_summary_blocks_archive_creation_and_keeps_original_copy() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        filesystem.put(object, LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        let candidate =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        filesystem.erase_existing(object);
        filesystem.put(object, b"third summary");
        let mutations = filesystem.snapshot().mutation_count();
        assert!(
            storage
                .finish_summary_migration(
                    &mut permit,
                    &candidate,
                    ARCHIVE,
                    &validator,
                    &operation,
                    &context
                )
                .is_err()
        );
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(storage.snapshot().stages(), 1);
        assert!(storage.snapshot().disposition_retained());
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        Ok(())
    })
}
