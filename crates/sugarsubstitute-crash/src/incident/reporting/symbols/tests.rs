// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual repository CAS with inert supplied facts, never fabricating a provider observation.
use super::*;
use crate::incident::tests::{
    publication_fixture::{admitted, setup},
    reporting_fixture::{TestResult, binding, installed, retained},
};
use sugarsubstitute_supervision::diagnostic_storage::fixture::*;
const PAYLOAD: &[u8] = b"original immutable symbol metadata fixture payload";
fn scenario() -> Result<(IncidentSummary, FailingFrame), Box<dyn std::error::Error + Send + Sync>> {
    let original = retained(PAYLOAD)?;
    let facts = original.facts();
    let dump = DumpId::new([7; 16])?;
    let summary = IncidentSummary::new(
        original.seed().clone(),
        original.revision(),
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            CaptureEvidence::Finished(dump),
            facts.payload().clone(),
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?;
    let release = ReleaseSymbols::new(
        summary.seed().build().clone(),
        FileLabel::new("host.exe")?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([1; 16], 1)?,
        [2; 32],
        [3; 32],
    );
    let frame = FailingFrame::new(
        dump,
        release,
        0x140001000,
        0x1000,
        SymbolName::new("fixture_exception_frame")?,
        Some(SourceLine::new(FileLabel::new("source.rs")?, 41)?),
    );
    Ok((summary, frame))
}
#[test]
fn symbol_metadata_real_cas_preserves_original_failure_capture_and_payload_generation() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let (original, frame) = scenario()?;
        let payload = DiagnosticObject::Payload(
            original.seed().key(),
            original.revision(),
            AttachmentKind::Fault,
        );
        fixture.put(payload, PAYLOAD)?;
        installed(&fixture, original.clone(), Some(original.revision()))?;
        let before =
            fixture.snapshot().active_identity(payload).ok_or("original payload absent")?;
        let identity = (original.seed().key(), original.revision(), frame.dump());
        let proof =
            repository.record_symbol_metadata(binding(&original)?, identity, &frame, &context)?;
        assert_eq!(proof.revision().get(), 3);
        assert_eq!(proof.payload_revision(), Some(original.revision()));
        assert_eq!(proof.summary().seed(), original.seed());
        assert_eq!(proof.summary().facts().retention(), original.facts().retention());
        assert_eq!(proof.summary().facts().exit(), original.facts().exit());
        assert_eq!(proof.summary().facts().failure(), original.facts().failure());
        assert_eq!(proof.summary().facts().capture(), original.facts().capture());
        assert_eq!(proof.summary().facts().payload(), original.facts().payload());
        assert_eq!(proof.summary().facts().reporting(), original.facts().reporting());
        assert_eq!(proof.summary().facts().symbols(), &SymbolOutcome::Resolved(frame));
        let loaded = repository.load(original.seed().key(), &context)?;
        assert_eq!(loaded.summary(), proof.summary());
        assert_eq!(loaded.payload_revision(), Some(original.revision()));
        let after = fixture.snapshot().active_identity(payload).ok_or("original payload lost")?;
        assert_eq!(before.file_identity(), after.file_identity());
        assert_eq!(before.sha256(), after.sha256());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Payload(
                    original.seed().key(),
                    proof.revision(),
                    AttachmentKind::Fault
                ))
                .is_none()
        );
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn symbol_metadata_stale_identity_dump_and_release_build_reject_without_mutation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let (original, frame) = scenario()?;
        installed(&fixture, original.clone(), Some(original.revision()))?;
        let before = fixture.snapshot().mutation_count();
        let identity = (original.seed().key(), original.revision(), frame.dump());
        let stale =
            ReportBinding::new(identity.0, IncidentRevision::new(1)?, ReportSessionId::new(3)?);
        assert!(matches!(
            repository.record_symbol_metadata(stale, identity, &frame, &context),
            Err(IncidentStoreError::StaleRevision)
        ));
        let current = binding(&original)?;
        let wrong_revision = (identity.0, IncidentRevision::new(1)?, identity.2);
        assert!(matches!(
            repository.record_symbol_metadata(current, wrong_revision, &frame, &context),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        let wrong_dump = (identity.0, identity.1, DumpId::new([8; 16])?);
        assert!(matches!(
            repository.record_symbol_metadata(current, wrong_dump, &frame, &context),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        let other_release = ReleaseSymbols::new(
            BuildIdentity::new("other_build")?,
            frame.release().module().clone(),
            frame.release().architecture(),
            frame.release().pdb(),
            *frame.release().image_sha256(),
            *frame.release().symbols_sha256(),
        );
        let wrong_build = FailingFrame::new(
            frame.dump(),
            other_release,
            frame.address(),
            frame.module_offset(),
            frame.symbol().clone(),
            frame.source().cloned(),
        );
        assert!(matches!(
            repository.record_symbol_metadata(current, identity, &wrong_build, &context),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn symbol_metadata_uncertain_activation_returns_original_native_error_without_commit() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let (original, frame) = scenario()?;
        installed(&fixture, original.clone(), Some(original.revision()))?;
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        let failure = repository
            .record_symbol_metadata(
                binding(&original)?,
                (original.seed().key(), original.revision(), frame.dump()),
                &frame,
                &context,
            )
            .err()
            .ok_or("uncertain activation fabricated a proof")?;
        assert!(std::error::Error::source(&failure).is_some());
        repository.retire(&context)?;
        Ok(())
    })
}
