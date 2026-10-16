// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inert acknowledgement fields cannot issue restart or remove an ordered effect without custody.
use super::*;
use sugarsubstitute_contracts::lifecycle::*;
fn binding() -> Result<(ReportBinding, RunKey)> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    };
    Ok((
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Desktop(run)),
            IncidentRevision::new(5)?,
            ReportSessionId::new(6)?,
        ),
        run,
    ))
}
#[test]
fn restart_original_next_ack_revision_and_subject_are_exact_correlation_requirements() -> Result<()>
{
    let (binding, run) = binding()?;
    let at = IncidentUtc::from_unix_seconds(7)?;
    let acknowledgement =
        DurableAcknowledgement::new(binding.incident(), binding.revision().next()?, at);
    assert!(validate_forward(binding, run, acknowledgement).is_ok());
    let stale = DurableAcknowledgement::new(binding.incident(), binding.revision(), at);
    assert!(validate_forward(binding, run, stale).is_err());
    let later =
        DurableAcknowledgement::new(binding.incident(), binding.revision().next()?.next()?, at);
    assert!(validate_forward(binding, run, later).is_err());
    let next = RunKey { generation: run.generation.next()?, ..run };
    assert!(validate_forward(binding, next, acknowledgement).is_err());
    let foreign = DurableAcknowledgement::new(
        IncidentKey::new(IncidentSubject::Desktop(next)),
        binding.revision().next()?,
        at,
    );
    assert!(validate_forward(binding, run, foreign).is_err());
    Ok(())
}
#[test]
fn restart_copied_ack_and_retirement_flags_preserve_whole_forward_without_native_receipt()
-> Result<()> {
    let (binding, run) = binding()?;
    let acknowledgement = DurableAcknowledgement::new(
        binding.incident(),
        binding.revision().next()?,
        IncidentUtc::from_unix_seconds(7)?,
    );
    let mut receiving = IncidentReporting::new();
    receiving.binding = Some(binding);
    receiving.family_retired = true;
    let final_request = DiagnosticRequestId::new(7)?;
    receiving.native_retirement.receive(final_request)?;
    assert!(receiving.native_retirement.consume_completion(final_request, true)?);
    receiving.dismiss_action = Some(ReportAction::DismissAndRequestRestart);
    receiving.retained_effect = Some(IncidentEffect::ForwardRestartIntent { run, acknowledgement });
    let mut host = IncidentHost::new();
    assert!(receiving.receiving_session.retired().is_none());
    assert!(!receiving.retain_current_restart(&mut host)?);
    assert!(receiving.restart_run(&mut host)?.is_none());
    assert!(matches!(receiving.retained_effect,
        Some(IncidentEffect::ForwardRestartIntent { run: original, acknowledgement: ack })
        if original == run && ack == acknowledgement));
    assert!(receiving.relay.is_none());
    assert!(receiving.normal_close_budget().is_none());
    Ok(())
}
