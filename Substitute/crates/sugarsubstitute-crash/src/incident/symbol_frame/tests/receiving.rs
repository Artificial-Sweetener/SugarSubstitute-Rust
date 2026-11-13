// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Semantic owner flow with bounded byte reads and inert external symbol/store facts.
//! Native committed-lease authentication, CNG, DbgHelp and process presentation are separate proofs.
use super::*;
use crate::incident::{StoredIncidentSummary, decode_summary, encode_summary};
use std::error::Error;
use sugarsubstitute_application::{incident::*, supervision::*};
use sugarsubstitute_contracts::{
    configuration::ProfileId, execution::Moment, incident::*, lifecycle::*, startup::LaunchId,
};

#[test]
fn unexplained_failed_exit_collection_summary_reaches_symbol_metadata_and_same_report_session()
-> Result<(), Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let process = ProcessKey { owner, id: ProcessId::new(5)? };
    let registration = ClientRegistration {
        run,
        connection: ConnectionId::new(1)?,
        launch: LaunchId::new(1)?,
        profile: ProfileId::new("Receiving_1".into())?,
    };
    let mut resources = ResourceCoordinator::new(owner);
    resources.enroll_run(RunEnrollment {
        run,
        launch: registration.launch,
        profile: registration.profile.clone(),
    })?;
    resources.register_run(registration)?;
    let mut coordinator = IncidentCoordinator::new(owner);
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("semantic-receiving")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::DesktopInitialization,
    )?;
    let seed_context = context(1, 100)?;
    let seed_effects = coordinator.seed(&mut resources, seed.clone(), seed_context)?;
    let Some(IncidentEffect::PublishSeed { scope: seed_scope, .. }) = seed_effects.iter().next()
    else {
        return Err("Original seed command missing".into());
    };
    coordinator.observe(IncidentServiceObservation::SeedCommitted {
        scope: *seed_scope,
        seed: seed.clone(),
    })?;
    let mut exit = DesktopExitTransaction::new(run, process)?;
    exit.observe(DesktopObservation::Exited { process, code: 0 })
        .map_err(|rejected| rejected.reason)?;
    let failed = exit.failed_incident()?;
    assert_eq!(failed.attribution(), DesktopExitKind::Unexpected);
    assert_eq!(failed.actual_status(), 0);
    let original = context(2, 101)?;
    let effects = coordinator.failed_desktop(
        &mut resources,
        failed,
        FailureEvidence::Unexplained,
        original,
    )?;
    let Some(IncidentEffect::CollectEvidence { scope, failure, attribution, .. }) =
        effects.iter().next()
    else {
        return Err("Actual failure collection command missing".into());
    };
    assert_eq!(*failure, FailureEvidence::Unexplained);
    assert_eq!(
        *attribution,
        ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 }
    );
    assert_eq!(scope.original(), original.budget());
    let collection_scope = *scope;
    let dump = DumpId::new([7; 16])?;
    let bytes = input();
    let digest: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&bytes.0).into();
    let attachment = AttachmentFacts::new(AttachmentKind::Minidump, 512, 512, 0, digest)?;
    let payload = PayloadDisposition::Retained(AttachmentInventory::new(&[attachment])?);
    let collected = CollectedIncidentEvidence::new(
        original.at(),
        CaptureEvidence::Finished(dump),
        payload,
        SymbolOutcome::NotRequested,
    );
    let effects = coordinator.observe(IncidentServiceObservation::EvidenceCollected {
        scope: collection_scope,
        evidence: collected,
    })?;
    let Some(IncidentEffect::PublishSummary { scope, summary }) = effects.iter().next() else {
        return Err("Actual collection summary command missing".into());
    };
    assert_eq!(*scope, collection_scope);
    let stored = StoredIncidentSummary::new(summary.clone(), Some(summary.revision()))?;
    let stored = decode_summary(&encode_summary(&stored)?)?;
    assert_eq!(stored.summary(), summary);
    assert_eq!(stored.summary().facts().failure(), FailureEvidence::Unexplained);
    let effects = coordinator.observe(IncidentServiceObservation::SummaryCommitted {
        scope: collection_scope,
        summary: stored.summary().clone(),
    })?;
    assert!(effects.iter().any(|effect| matches!(effect, IncidentEffect::RetireSource { .. })));
    let (binding, before) = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::LaunchReporter { binding, summary } => {
                Some((*binding, summary.clone()))
            }
            _ => None,
        })
        .ok_or("Collected summary did not admit its Reporter")?;
    let frame = extract(&mut input(), 512, Some(0), stored.summary().facts().failure())
        .map_err(|cause| std::io::Error::other(format!("actual bounded extractor: {cause:?}")))?;
    assert_eq!((frame.code(), frame.address()), (0xc0000005, 0x1008));
    assert_eq!(before.facts().exit(), *attribution);
    assert_eq!(before.facts().failure(), FailureEvidence::Unexplained);
    let symbol_scope = coordinator.admit_report_symbols(binding, context(3, 102)?)?;
    let resolved = FailingFrame::new(
        dump,
        ReleaseSymbols::new(
            before.seed().build().clone(),
            FileLabel::new("SugarSubstitute.exe")?,
            SymbolArchitecture::X86_64,
            frame.pdb(),
            [8; 32],
            [9; 32],
        ),
        frame.address(),
        frame.address() - frame.image_base(),
        SymbolName::new("inert_external_symbol_fact")?,
        None,
    );
    let facts = before.facts();
    let successor = IncidentSummary::new(
        before.seed().clone(),
        before.revision().next()?,
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            facts.reporting(),
            SymbolOutcome::Resolved(resolved),
        ),
    )?;
    let successor = decode_summary(&encode_summary(&StoredIncidentSummary::new(
        successor,
        stored.payload_revision(),
    )?)?)?;
    let observation = IncidentServiceObservation::SymbolsCommitted {
        scope: symbol_scope,
        binding,
        summary: successor.summary().clone(),
    };
    let effects = coordinator.observe(observation.clone())?;
    let Some(IncidentEffect::LaunchReporter { binding: current, summary }) = effects.iter().next()
    else {
        return Err("Actual symbol metadata did not restore Reporter admission".into());
    };
    assert_eq!(current.incident(), binding.incident());
    assert_eq!(current.session(), binding.session());
    assert_eq!(current.revision(), binding.revision().next()?);
    assert_eq!(summary.facts().failure(), FailureEvidence::Unexplained);
    assert_eq!(summary.facts().exit(), before.facts().exit());
    assert_eq!(summary.facts().payload(), before.facts().payload());
    assert_eq!(successor.payload_revision(), stored.payload_revision());
    assert!(coordinator.observe(observation)?.is_empty());
    assert!(coordinator.admit_report_view(*current, context(4, 103)?).is_ok());
    Ok(())
}
fn context(nanos: u64, utc: i64) -> Result<IncidentRequestContext, Box<dyn Error>> {
    Ok(IncidentRequestContext::new(
        IncidentOperationBudget::new(Moment::from_nanos(nanos))?,
        IncidentUtc::from_unix_seconds(utc)?,
    ))
}
