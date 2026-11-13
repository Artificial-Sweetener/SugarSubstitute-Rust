// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify current-view action and durable-close causality without native effects.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
fn binding(revision: u64, session: u64) -> Result<ReportBinding, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(revision)?,
        ReportSessionId::new(session)?,
    ))
}
fn ready() -> Result<State, Box<dyn Error>> {
    let mut state = State::new(
        binding(1, 1)?,
        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
    );
    state.presented()?;
    state.preflight_replace(binding(1, 1)?, binding(2, 1)?)?;
    state.replaced(
        binding(2, 1)?,
        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
    );
    Ok(state)
}
#[test]
fn presented_alone_cannot_admit_actions_before_actual_replacement() -> Result<(), Box<dyn Error>> {
    let mut state = State::new(
        binding(1, 1)?,
        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
    );
    assert!(state.request(binding(1, 1)?, Intent::Action(ReportAction::CopyText)).is_err());
    state.presented()?;
    assert!(state.presented().is_err());
    assert!(state.request(binding(1, 1)?, Intent::Action(ReportAction::Dismiss)).is_err());
    state.preflight_replace(binding(1, 1)?, binding(2, 1)?)?;
    assert!(!state.actionable());
    state.replaced(
        binding(2, 1)?,
        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
    );
    assert!(state.actionable());
    Ok(())
}
#[test]
fn foreign_stale_duplicate_replacements_preserve_current_view() -> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    for (old, current) in [
        (binding(1, 1)?, binding(3, 1)?),
        (binding(2, 1)?, binding(2, 1)?),
        (binding(2, 1)?, binding(1, 1)?),
        (binding(2, 1)?, binding(3, 2)?),
    ] {
        assert!(state.preflight_replace(old, current).is_err());
        assert_eq!(state.binding(), binding(2, 1)?);
        assert!(state.actionable());
    }
    Ok(())
}
#[test]
fn one_request_fences_duplicate_input_and_correlates_result() -> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    assert!(state.request(binding(1, 1)?, Intent::Action(ReportAction::CopyText)).is_err());
    assert!(
        state
            .request(
                binding(2, 1)?,
                Intent::Action(ReportAction::Export(ExportSelection::TextOnly))
            )
            .is_err()
    );
    let first = state.request(binding(2, 1)?, Intent::Action(ReportAction::CopyText))?;
    assert_eq!(first.id.get(), 1);
    assert!(state.request(binding(2, 1)?, Intent::Action(ReportAction::Dismiss)).is_err());
    assert!(state.matching_request(first.id, binding(3, 1)?).is_err());
    assert!(
        state
            .matching_request(ReportRequestId(NonZeroU64::new(2).ok_or("zero")?), binding(2, 1)?)
            .is_err()
    );
    state.matching_request(first.id, binding(2, 1)?)?;
    state.completed_request();
    assert!(state.matching_request(first.id, binding(2, 1)?).is_err());
    assert_eq!(
        state.request(binding(2, 1)?, Intent::Export(ExportSelection::TextOnly))?.id.get(),
        2
    );
    Ok(())
}
#[test]
fn request_and_replacement_exhaustion_preflight_before_mutation() -> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    state.high_water = u64::MAX;
    assert!(state.request(binding(2, 1)?, Intent::Action(ReportAction::Dismiss)).is_err());
    assert!(state.pending.is_none());
    state.replacements = u64::MAX;
    assert!(state.preflight_replace(binding(2, 1)?, binding(3, 1)?).is_err());
    assert!(state.actionable());
    Ok(())
}
#[test]
fn only_matching_dismiss_and_later_actual_ack_authorize_close() -> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    let ack = DurableAcknowledgement::new(
        binding(2, 1)?.incident(),
        IncidentRevision::new(3)?,
        IncidentUtc::from_unix_seconds(10)?,
    );
    assert!(state.durable_close(binding(2, 1)?, ack).is_err());
    state.request(binding(2, 1)?, Intent::Action(ReportAction::CopyText))?;
    assert!(state.durable_close(binding(2, 1)?, ack).is_err());
    state.completed_request();
    state.request(binding(2, 1)?, Intent::Action(ReportAction::DismissAndRequestRestart))?;
    let old = DurableAcknowledgement::new(ack.incident(), IncidentRevision::new(2)?, ack.at());
    assert!(state.durable_close(binding(2, 1)?, old).is_err());
    assert!(state.durable_close(binding(2, 2)?, ack).is_err());
    assert!(!state.closing());
    state.durable_close(binding(2, 1)?, ack)?;
    assert!(state.closing());
    assert!(!state.actionable());
    assert!(state.durable_close(binding(2, 1)?, ack).is_err());
    Ok(())
}
#[test]
fn close_request_is_not_native_retirement_or_a_receipt() -> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    assert!(state.native_retired().is_err());
    state.shutdown();
    assert!(state.receipt().is_err());
    let mut state = ready()?;
    state.shutdown();
    state.native_retired()?;
    assert!(state.native_retired().is_err());
    let receipt = state.receipt()?;
    assert!(receipt.presented());
    assert_eq!(receipt.replacements(), 1);
    Ok(())
}

#[test]
fn unavailable_restart_keeps_copy_export_and_dismiss_actionable_without_a_request()
-> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    state.restart = ReportRestartAvailability::Unavailable;
    let current = state.binding();
    let restart = Intent::Action(ReportAction::DismissAndRequestRestart);
    assert!(!restart.allowed(state.restart()));
    assert!(state.request(current, restart).is_err());
    assert_eq!(state.high_water, 0);
    assert!(state.pending.is_none());
    assert!(state.actionable());
    for intent in [
        Intent::Action(ReportAction::CopyText),
        Intent::Export(ExportSelection::TextOnly),
        Intent::Action(ReportAction::Dismiss),
    ] {
        assert!(intent.allowed(state.restart()));
        state.request(current, intent)?;
        state.completed_request();
    }
    assert!(!state.closing());
    Ok(())
}
#[test]
fn restart_rejection_requires_exact_pending_restart_and_never_closes_the_view()
-> Result<(), Box<dyn Error>> {
    let mut state = ready()?;
    let current = state.binding();
    let copy = state.request(current, Intent::Action(ReportAction::CopyText))?;
    assert!(state.restart_unavailable(copy.id, current).is_err());
    assert!(state.matching_request(copy.id, current).is_ok());
    state.completed_request();
    let request = state.request(current, Intent::Action(ReportAction::DismissAndRequestRestart))?;
    assert!(state.restart_unavailable(request.id, binding(3, 1)?).is_err());
    assert!(
        state
            .restart_unavailable(
                ReportRequestId(NonZeroU64::new(request.id.get() + 1).ok_or("zero")?),
                current
            )
            .is_err()
    );
    assert!(state.restart().allows_restart());
    state.restart_unavailable(request.id, current)?;
    assert!(!state.restart().allows_restart());
    assert!(!state.closing());
    assert!(state.actionable());
    assert!(state.restart_unavailable(request.id, current).is_err());
    assert!(
        state.request(current, Intent::Action(ReportAction::DismissAndRequestRestart)).is_err()
    );
    state.request(current, Intent::Action(ReportAction::Dismiss))?;
    Ok(())
}
