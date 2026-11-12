// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact recovered durable intent never supplies a current registered startup owner.
use super::*;
fn inert_binding() -> Result<ReportBinding> {
    use sugarsubstitute_contracts::lifecycle::*;
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(5)?,
        ReportSessionId::new(6)?,
    ))
}
#[test]
fn recovered_intent_rejects_wrong_ack_or_absent_current_source_without_losing_effect() -> Result<()>
{
    let binding = inert_binding()?;
    let at = IncidentUtc::from_unix_seconds(100)?;
    let ack = DurableAcknowledgement::new(binding.incident(), binding.revision().next()?, at);
    assert!(validate_recovered_forward(binding, binding.incident(), ack).is_ok());
    let stale = DurableAcknowledgement::new(binding.incident(), binding.revision(), at);
    assert!(validate_recovered_forward(binding, binding.incident(), stale).is_err());
    let effect = IncidentEffect::ForwardRecoveredRestartIntent {
        incident: binding.incident(),
        acknowledgement: ack,
    };
    let mut receiving = IncidentReporting::new();
    receiving.retained_effect = Some(effect);
    assert!(
        receiving
            .target(binding, receiving.retained_effect.as_ref().ok_or("Intent absent")?)
            .is_err()
    );
    assert!(
        matches!(receiving.retained_effect, Some(IncidentEffect::ForwardRecoveredRestartIntent { acknowledgement, .. }) if acknowledgement == ack)
    );
    assert!(receiving.recovery_source.is_none());
    Ok(())
}
#[test]
fn old_live_run_forward_guard_does_not_accept_recovered_supervisor_ack_as_run_ownership()
-> Result<()> {
    let binding = inert_binding()?;
    let owner = binding.incident().subject().coordinator();
    let run = RunKey {
        owner,
        instance: sugarsubstitute_contracts::lifecycle::InstanceId::new(1)?,
        generation: sugarsubstitute_contracts::lifecycle::RunGeneration::new(1)?,
    };
    let ack = DurableAcknowledgement::new(
        binding.incident(),
        binding.revision().next()?,
        IncidentUtc::from_unix_seconds(100)?,
    );
    assert_ne!(binding.incident().subject(), IncidentSubject::Desktop(run));
    assert!(validate_forward(binding, run, ack).is_err());
    Ok(())
}
