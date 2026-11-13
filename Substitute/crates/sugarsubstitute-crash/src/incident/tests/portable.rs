// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify preserved committed history without changing active native authority.
use super::{portable_fixture::*, *};
use crate::incident::portable::*;
use sugarsubstitute_supervision::diagnostic_storage::{DiagnosticEntryClass, DiagnosticObject};
mod archive;
mod current_store;
mod exporting;

#[test]
fn prelaunch_origin_never_proves_a_completed_capture() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let seed = base_summary()?.summary().seed().clone();
        let IncidentSubject::Desktop(run) = seed.key().subject() else {
            return Err("Expected desktop source".into());
        };
        let operation = LogicalStoreOperationKey::new(run.owner, IncidentOperationId::new(5)?);
        let origin = crate::incident::RecoveryCaptureOriginRecord::new(
            operation,
            sugarsubstitute_contracts::crash::RecoveryCaptureIdentity::new(
                run,
                sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantId::new(6)?,
            ),
            [7; 24],
            seed.build(),
        )?;
        source.add(
            DiagnosticEntryClass::Journal,
            Some(DiagnosticObject::Journal(operation)),
            encode_store_state(&IncidentStoreState::RecoveryCaptureOrigin(origin))?,
        )?;
        let original = source.original_bytes();
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Prelaunch origin became committed history".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::UnsupportedRecord);
        drop(failure);
        assert!(source.unchanged(&original));
        Ok(())
    })
}

#[test]
fn copied_current_summary_reopens_with_original_attribution_and_unchanged_bytes() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let stored = base_summary()?;
        source.summary(stored.clone())?;
        let bytes = source.original_bytes();
        let old_ids = source.rows.iter().map(|row| row.physical).collect::<Vec<_>>();
        let mut source = source.copied();
        assert!(source.rows.iter().map(|row| row.physical).ne(old_ids));
        for _ in 0..2 {
            let history = project_portable_committed(
                &mut source,
                NamespaceId::new([1; 32]),
                now()?,
                &context,
            )?;
            assert_eq!(history.summaries().collect::<Vec<_>>(), vec![&stored]);
            assert_eq!(history.provenance().namespace(), NamespaceId::new([1; 32]));
            drop(history);
        }
        assert!(source.unchanged(&bytes));
        Ok(())
    })
}

#[test]
fn foreign_source_namespace_and_foreign_summary_are_refused() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([9; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Foreign provenance accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::ForeignNamespace);
        drop(failure);
        let old = base_summary()?;
        let seed = IncidentSeed::new(
            IncidentKey::new(IncidentSubject::Desktop(RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([9; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(3)?,
                generation: RunGeneration::new(4)?,
            })),
            IncidentRevision::new(1)?,
            BuildIdentity::new("build_1")?,
            IncidentUtc::from_unix_seconds(100)?,
            IncidentStage::Startup,
        )?;
        let foreign =
            IncidentSummary::new(seed, old.summary().revision(), old.summary().facts().clone())?;
        source.summary(StoredIncidentSummary::new(foreign, None)?)?;
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Foreign record accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::ForeignNamespace);
        Ok(())
    })
}

#[test]
fn corrupt_unknown_unassociated_and_duplicate_entries_preserve_originals() -> TestResult {
    admitted(|context| {
        for case in 0..4 {
            let mut source = Source::new()?;
            let expected = match case {
                0 => {
                    source.add(DiagnosticEntryClass::Summary, None, b"{broken".to_vec())?;
                    PortableIncidentFailureCode::Codec
                }
                1 => {
                    source.add(
                        DiagnosticEntryClass::Unknown,
                        None,
                        b"preserved unknown".to_vec(),
                    )?;
                    PortableIncidentFailureCode::UnsupportedRecord
                }
                2 => {
                    let seed = base_summary()?.summary().seed().clone();
                    source.add(
                        DiagnosticEntryClass::Seed,
                        Some(DiagnosticObject::Seed(seed.key())),
                        encode_seed(&seed)?,
                    )?;
                    PortableIncidentFailureCode::Uncommitted
                }
                _ => {
                    source.duplicate_inventory();
                    PortableIncidentFailureCode::Duplicate
                }
            };
            let bytes = source.original_bytes();
            let failure = match project_portable_committed(
                &mut source,
                NamespaceId::new([1; 32]),
                now()?,
                &context,
            ) {
                Ok(_) => return Err("Invalid source accepted".into()),
                Err(cause) => cause,
            };
            assert_eq!(failure.code(), expected);
            if case == 0 {
                assert!(std::error::Error::source(&failure).is_some());
            }
            drop(failure);
            assert!(source.unchanged(&bytes));
        }
        Ok(())
    })
}

#[test]
fn truncated_actual_record_and_original_deadline_refuse_without_publication() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        source.short = true;
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Truncated record accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::WrongDigest);
        drop(failure);
        source.short = false;
        source.reads = 0;
        let expired = context.bounded_until(context.now());
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &expired,
        ) {
            Ok(_) => return Err("Expired original context accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::Deadline);
        assert_eq!(source.reads, 0);
        Ok(())
    })
}
