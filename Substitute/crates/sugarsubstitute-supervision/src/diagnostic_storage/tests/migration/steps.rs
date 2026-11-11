// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the selected typed step and current-scan source classification through real storage phases.
use super::*;
use sugarsubstitute_contracts::incident::SummaryMigrationStep;
#[test]
fn typed_two_to_three_plan_survives_archive_and_stale_scan_or_changed_step_cannot_admit()
-> TestResult {
    for changed in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let mut original =
                storage.read(DiagnosticObject::Summary(incident()?), 64, &operation, &context)?;
            let mut preparation =
                storage.prepare_summary_migration(&mut original, &operation, &context)?;
            preparation.write_candidate(CANDIDATE, &context)?;
            let candidate = preparation.flush_candidate(&context)?.read_facts();
            let plan = DiagnosticSummaryMigrationPlan::for_step(
                SummaryMigrationStep::TwoToThree,
                operation.key,
                incident()?,
                IncidentRevision::new(1)?,
                None,
                recorded(preparation.original_file())?,
                recorded(preparation.original_stage().read_facts())?,
                recorded(candidate)?,
                Sha256::digest(JOURNAL).into(),
            )?;
            let validator = Validator::new(plan);
            if changed {
                let old = DiagnosticSummaryMigrationPlan::new(
                    plan.historical_operation(),
                    plan.incident(),
                    plan.revision(),
                    plan.payload_revision(),
                    plan.original_file(),
                    plan.original_stage(),
                    plan.candidate_stage(),
                    plan.journal_sha256(),
                )?;
                assert_eq!(old.step(), SummaryMigrationStep::OneToTwo);
                let mutations = filesystem.snapshot().mutation_count();
                assert!(
                    storage
                        .seal_summary_migration(
                            &mut preparation,
                            old,
                            &validator,
                            &operation,
                            &context
                        )
                        .is_err()
                );
                assert_eq!(filesystem.snapshot().mutation_count(), mutations);
            } else {
                let mut journal = storage.seal_summary_migration(
                    &mut preparation,
                    plan,
                    &validator,
                    &operation,
                    &context,
                )?;
                journal.write_chunk(JOURNAL, &context)?;
                journal.flush(&context)?;
                let mut permit =
                    storage.activate_summary_migration(&mut journal, &operation, &context)?;
                assert_eq!(permit.plan().step(), SummaryMigrationStep::TwoToThree);
                let actual = storage.complete_summary_migration(
                    &mut permit,
                    &validator,
                    &operation,
                    &context,
                )?;
                storage.finish_summary_migration(
                    &mut permit,
                    &actual,
                    ARCHIVE,
                    &validator,
                    &operation,
                    &context,
                )?;
                storage.inventory(&operation, &context)?;
                assert!(
                    storage.validate_preserved_migration_inventory(&operation, &context).is_err()
                );
                let mut archive = storage.read(
                    DiagnosticObject::Journal(operation.key),
                    64,
                    &operation,
                    &context,
                )?;
                storage.recognize_summary_migration_archive(
                    &mut archive,
                    &validator,
                    &operation,
                    &context,
                )?;
                storage.validate_preserved_migration_inventory(&operation, &context)?;
                let mut repeated = storage.read(
                    DiagnosticObject::Summary(incident()?),
                    64,
                    &operation,
                    &context,
                )?;
                let effects = filesystem.snapshot().mutation_count();
                assert!(
                    storage.prepare_summary_migration(&mut repeated, &operation, &context).is_err()
                );
                assert_eq!(filesystem.snapshot().mutation_count(), effects);
                repeated.retire(&context)?;

                let _unknown = filesystem.put_stage(
                    DiagnosticObject::Summary(incident()?),
                    key(77)?,
                    b"unrecognized preserved bytes",
                );
                storage.inventory(&operation, &context)?;
                let mut archive = storage.read(
                    DiagnosticObject::Journal(operation.key),
                    64,
                    &operation,
                    &context,
                )?;
                storage.recognize_summary_migration_archive(
                    &mut archive,
                    &validator,
                    &operation,
                    &context,
                )?;
                assert!(
                    storage.validate_preserved_migration_inventory(&operation, &context).is_err()
                );
            }
            storage.retire(&operation, &context)?;
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
