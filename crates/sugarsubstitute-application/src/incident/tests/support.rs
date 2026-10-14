// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project external store/reporter facts while exercising the real policy owners.
pub(super) use super::super::*;
pub(super) use crate::supervision::*;
use sugarsubstitute_contracts::{configuration::ProfileId, startup::LaunchId};
pub(super) use sugarsubstitute_contracts::{execution::Moment, incident::*, lifecycle::*};
pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(super) fn owner(epoch: u8) -> Result<CoordinatorKey, IdentityError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([epoch; 16])?,
    })
}
pub(super) fn run(instance: u64) -> Result<RunKey, IdentityError> {
    Ok(RunKey {
        owner: owner(2)?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(1)?,
    })
}
pub(super) fn context(
    nanos: u64,
    utc: i64,
) -> Result<IncidentRequestContext, Box<dyn std::error::Error>> {
    Ok(IncidentRequestContext::new(
        IncidentOperationBudget::new(Moment::from_nanos(nanos))?,
        IncidentUtc::from_unix_seconds(utc)?,
    ))
}
pub(super) fn seed(run: RunKey) -> Result<IncidentSeed, IncidentError> {
    IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build01")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::DesktopInitialization,
    )
}
pub(super) fn register(resources: &mut ResourceCoordinator, run: RunKey) -> TestResult {
    let registration = ClientRegistration {
        run,
        launch: LaunchId::new(run.instance.get())?,
        profile: ProfileId::new(format!("profile{}", run.instance.get()))?,
        connection: ConnectionId::new(run.instance.get())?,
    };
    resources.enroll_run(RunEnrollment {
        run: registration.run,
        launch: registration.launch,
        profile: registration.profile.clone(),
    })?;
    resources.register_run(registration)?;
    Ok(())
}
pub(super) fn seed_ready(
    coord: &mut IncidentCoordinator,
    resources: &mut ResourceCoordinator,
    seed: IncidentSeed,
) -> Result<IncidentOperationScope, Box<dyn std::error::Error>> {
    let effects = coord.seed(resources, seed.clone(), context(1, 100)?)?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::PublishSeed { scope, .. }) => *scope,
        _ => return Err("Seed effect missing".into()),
    };
    assert!(coord.observe(IncidentServiceObservation::SeedCommitted { scope, seed })?.is_empty());
    Ok(scope)
}
pub(super) fn exit(
    run: RunKey,
    kind: Option<CompletionIntent>,
) -> Result<DesktopExitTransaction, Box<dyn std::error::Error>> {
    let process = ProcessKey { owner: run.owner, id: ProcessId::new(run.instance.get())? };
    let mut tx = DesktopExitTransaction::new(run, process)?;
    if let Some(intent) = kind {
        tx.begin(run, ShutdownId::new(1)?, intent, 0, Moment::from_nanos(0))?;
        tx.observe(DesktopObservation::Completed { run, shutdown: ShutdownId::new(1)? })
            .map_err(|rejected| rejected.reason)?;
    }
    tx.observe(DesktopObservation::Exited { process, code: 0 })
        .map_err(|rejected| rejected.reason)?;
    Ok(tx)
}
pub(super) fn evidence() -> Result<CollectedIncidentEvidence, IncidentError> {
    Ok(CollectedIncidentEvidence::new(
        IncidentUtc::from_unix_seconds(101)?,
        CaptureEvidence::Unavailable(CaptureAbsence::Missing),
        PayloadDisposition::Unavailable,
        SymbolOutcome::NotRequested,
    ))
}
pub(super) struct Ready {
    pub coord: IncidentCoordinator,
    pub resources: ResourceCoordinator,
    pub seed: IncidentSeed,
    pub source: IncidentOperationScope,
    pub binding: ReportBinding,
    pub summary: IncidentSummary,
}
pub(super) fn ready(instance: u64) -> Result<Ready, Box<dyn std::error::Error>> {
    ready_with_evidence(instance, evidence()?)
}
pub(super) fn ready_with_evidence(
    instance: u64,
    collected: CollectedIncidentEvidence,
) -> Result<Ready, Box<dyn std::error::Error>> {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let run = run(instance)?;
    register(&mut resources, run)?;
    let seed = seed(run)?;
    seed_ready(&mut coord, &mut resources, seed.clone())?;
    let fact = exit(run, None)?.failed_incident()?;
    let effects = coord.failed_desktop(
        &mut resources,
        fact,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    let source = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("Collection effect missing".into()),
    };
    let effects = coord.observe(IncidentServiceObservation::EvidenceCollected {
        scope: source,
        evidence: collected,
    })?;
    let summary = match effects.iter().next() {
        Some(IncidentEffect::PublishSummary { summary, .. }) => summary.clone(),
        _ => return Err("Summary effect missing".into()),
    };
    let effects = coord.observe(IncidentServiceObservation::SummaryCommitted {
        scope: source,
        summary: summary.clone(),
    })?;
    let binding = effects
        .iter()
        .find_map(|e| {
            if let IncidentEffect::LaunchReporter { binding, .. } = e {
                Some(*binding)
            } else {
                None
            }
        })
        .ok_or("Reporter effect missing")?;
    Ok(Ready { coord, resources, seed, source, binding, summary })
}
pub(super) fn changed(
    summary: &IncidentSummary,
    reporting: ReportDisposition,
    ack: Option<IncidentUtc>,
) -> Result<IncidentSummary, IncidentError> {
    let facts = summary.facts();
    IncidentSummary::new(
        summary.seed().clone(),
        summary.revision().next()?,
        SummaryFacts::new(
            RetentionFacts::new(facts.retention().occurred(), ack)?,
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            reporting,
            facts.symbols().clone(),
        ),
    )
}
pub(super) fn present(ready: &mut Ready) -> TestResult {
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: ready.binding,
        context: context(3, 102)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("Attempt effect missing".into()),
    };
    ready.summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: ready.binding,
        summary: ready.summary.clone(),
    })?;
    ready.binding = match effects.iter().next() {
        Some(IncidentEffect::ReplaceReportView { current, .. }) => *current,
        _ => return Err("Refresh effect missing".into()),
    };
    ready
        .coord
        .observe(IncidentServiceObservation::ReportViewReplaced { binding: ready.binding })?;
    Ok(())
}
pub(super) fn acknowledge(
    ready: &mut Ready,
    restart: bool,
) -> Result<IncidentEffects, Box<dyn std::error::Error>> {
    let ctx = context(4, 103)?;
    let action =
        if restart { ReportAction::DismissAndRequestRestart } else { ReportAction::Dismiss };
    let IncidentReportDecision::Effects(effects) =
        ready.coord.report_action(ReportCommand::new(ready.binding, action), ctx)?
    else {
        return Err("Ack effect missing".into());
    };
    let scope = match effects.iter().next() {
        Some(IncidentEffect::Acknowledge { scope, .. }) => *scope,
        _ => return Err("Ack effect missing".into()),
    };
    ready.summary = changed(&ready.summary, ready.summary.facts().reporting(), Some(ctx.at()))?;
    Ok(ready.coord.observe(IncidentServiceObservation::Acknowledged {
        scope,
        binding: ready.binding,
        acknowledgement: DurableAcknowledgement::new(
            ready.seed.key(),
            ready.summary.revision(),
            ctx.at(),
        ),
        summary: ready.summary.clone(),
    })?)
}
pub(super) fn historical(
    instance: u64,
    ack: bool,
) -> Result<IncidentSummary, Box<dyn std::error::Error>> {
    let mut run = run(instance)?;
    run.owner = owner(3)?;
    let seed = seed(run)?;
    Ok(IncidentSummary::new(
        seed,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(
                IncidentUtc::from_unix_seconds(101)?,
                if ack { Some(IncidentUtc::from_unix_seconds(102)?) } else { None },
            )?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Unavailable,
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?)
}
pub(super) fn recovery(
    coord: &mut IncidentCoordinator,
) -> Result<IncidentRecoveryScope, Box<dyn std::error::Error>> {
    let effects = coord.begin_recovery(context(20, 120)?.budget())?;
    match effects.iter().next() {
        Some(IncidentEffect::Recover { scope }) => Ok(*scope),
        _ => Err("Recovery effect missing".into()),
    }
}
pub(super) fn historical_operation() -> Result<LogicalStoreOperationKey, Box<dyn std::error::Error>>
{
    Ok(LogicalStoreOperationKey::new(owner(3)?, IncidentOperationId::new(7)?))
}
