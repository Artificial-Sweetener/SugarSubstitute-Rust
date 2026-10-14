// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream actual native-source ownership into the genuine protected repository transaction.
use super::super::*;
use super::{
    abi_boundary, runtime,
    support::{Harness, TestError},
};
use crate::incident::*;
use std::path::Path;
use sugarsubstitute_contracts::incident::*;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
fn run(partial: bool) -> Result<(), TestError> {
    let mut harness = Harness::new()?;
    let mut source = runtime::create(&mut harness)?;
    let client = harness.client.clone();
    let (mut source, mut repository, mut token, fixture) =
        harness.run(2, 8 * 1024 * 1024, move |context| {
            let owner = runtime::owner()?;
            let fixture = DiagnosticStorageFixture::new()?;
            let ledger = SummaryEvictions::new(owner.namespace, 0, 0, 0, None)?;
            fixture.put(
                DiagnosticObject::StoreState,
                &encode_store_state(&IncidentStoreState::Ledger(ledger))?,
            )?;
            let mut storage = fixture.storage(&context)?;
            let key = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?);
            let operation = storage.begin_operation(key, &context)?;
            storage.resolve(
                Path::new("external-repository-fixture"),
                owner.namespace,
                &operation,
                &context,
            )?;
            storage.acquire_writer(&operation, &context)?;
            storage.inventory(&operation, &context)?;
            let mut generation = None;
            storage.prepare_capture_read(
                source.root_mut(),
                &mut generation,
                &operation,
                &context,
            )?;
            let (cpp, rust) = storage.reserve_finished_owner(
                generation.as_mut().ok_or("no generation")?,
                &operation,
                &context,
            )?;
            let (mut cpp, mut rust, mut output) = (Some(cpp), Some(rust), None);
            Token::install(
                &mut output,
                abi_boundary::api(),
                &mut generation,
                &mut cpp,
                &mut rust,
                &context,
            )
            .map_err(|e| format!("install {e:?}"))?;
            let mut token = output.ok_or("no token")?;
            token.encode_deadline(&client).map_err(|e| format!("deadline {e:?}"))?;
            let build = BuildIdentity::new("copy")?;
            let incident = IncidentKey::new(IncidentSubject::Supervisor(owner));
            assert_eq!(token.open(incident, &build, None, &context), Ok(0));
            let raw = token.fields.as_ref().ok_or("no fields")?.state.token;
            let mut repository = IncidentRepository::new(storage, operation, &context)?;
            let seed = IncidentSeed::new(
                incident,
                IncidentRevision::new(1)?,
                build,
                IncidentUtc::from_unix_seconds(100)?,
                IncidentStage::Running,
            )?;
            let preflight = repository.preflight_publication(
                seed.clone(),
                ExpectedSummary::Absent,
                IncidentPublicationDemand::new(64, 1, 1, 131072)?,
                IncidentUtc::from_unix_seconds(101)?,
                &context,
            )?;
            let mut ticket = repository.begin_publish(preflight, key.id(), &context)?;
            let mut copy = None;
            if partial {
                fixture.set_failure(DiagnosticFixtureFailure::PartialWrite, true);
            }
            let result =
                repository.append_finished_token(&mut ticket, &mut token, &mut copy, &context);
            if partial {
                assert!(result.is_err());
                let before = fixture.snapshot().mutation_count();
                fixture.set_failure(DiagnosticFixtureFailure::PartialWrite, false);
                assert!(matches!(
                    repository.append_finished_token(&mut ticket, &mut token, &mut copy, &context),
                    Err(IncidentStoreError::RecoveryConflict)
                ));
                assert_eq!(fixture.snapshot().mutation_count(), before);
                assert_eq!(abi_boundary::counts(raw).0, 1);
            } else {
                let fact = result?;
                assert_eq!(fact.retained_bytes(), 64);
                let before = fixture.snapshot().mutation_count();
                assert_eq!(
                    repository.append_finished_token(
                        &mut ticket,
                        &mut token,
                        &mut copy,
                        &context
                    )?,
                    fact
                );
                assert_eq!(fixture.snapshot().mutation_count(), before);
                assert_eq!(abi_boundary::counts(raw).0, 1);
                token.prepare(&context).map_err(|e| format!("prepare {e:?}"))?;
                let summary = IncidentSummary::new(
                    seed,
                    IncidentRevision::new(2)?,
                    SummaryFacts::new(
                        RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
                        ExitAttribution::Supervisor { status: 17 },
                        FailureEvidence::Unexplained,
                        CaptureEvidence::Finished(DumpId::new([5; 16])?),
                        PayloadDisposition::Retained(AttachmentInventory::new(&[fact])?),
                        ReportDisposition::new(0, None)?,
                        SymbolOutcome::NotRequested,
                    ),
                )?;
                let committed = repository.commit(&mut ticket, summary.clone(), &context)?;
                assert_eq!(committed.summary(), &summary);
                assert_eq!(committed.payload_revision(), Some(summary.revision()));
                let loaded = repository.load(incident, &context)?;
                assert_eq!(loaded.summary(), &summary);
                assert_eq!(loaded.payload_revision(), Some(summary.revision()));
                drop(loaded);
                let mut journal = None;
                repository.publish_capture_journal_token(
                    &mut token,
                    &mut journal,
                    &committed,
                    &context,
                )?;
                fixture.set_failure(DiagnosticFixtureFailure::AbsenceObservation, true);
                assert!(
                    repository
                        .settle_capture_journal_token(&mut token, &mut journal, &context)
                        .is_err()
                );
                assert_eq!(abi_boundary::dispositions(raw), 1);
                fixture.set_failure(DiagnosticFixtureFailure::AbsenceObservation, false);
                repository.settle_capture_journal_token(&mut token, &mut journal, &context)?;
                assert_eq!(abi_boundary::dispositions(raw), 1);
                assert!(
                    fixture.snapshot().active_identity(DiagnosticObject::Journal(key)).is_none()
                );
                assert_eq!(repository.snapshot().reads(), 1);
            }
            Ok((source, repository, token, fixture))
        })?;
    let client = harness.client.clone();
    let retained = harness.run(3, 32768, move |context| {
        assert_eq!(token.retire_cpp(&client, &context), Ok(true));
        token.retire_source(&context).map_err(|e| format!("source {e:?}"))?;
        repository.retire(&context)?;
        source.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok((source, repository, token, fixture))
    })?;
    harness.join();
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    drop(retained);
    assert_eq!(harness.main.snapshot().reserved_bytes, 0);
    assert_eq!(harness.source.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn whole_finished_stream_is_published_once_with_immutable_payload_generation()
-> Result<(), TestError> {
    run(false)
}
#[test]
fn partial_finished_write_preserves_stage_and_never_replays_uncertain_bytes()
-> Result<(), TestError> {
    run(true)
}
