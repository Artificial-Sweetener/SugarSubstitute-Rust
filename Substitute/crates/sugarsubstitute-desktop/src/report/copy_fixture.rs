// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Produce inert native action request metadata through the real pure surface state for owner tests.
//!
//! This default-off boundary fixture creates no window, handles, native observation,
//! authentication, clipboard effects or presentation proof. It preserves the private
//! request constructor and exercises the same State transition and request allocation.
use super::{
    ReportRequestId, ReportSurfaceError,
    state::{Intent, State},
};
use sugarsubstitute_contracts::incident::{ReportAction, ReportBinding, ReportCommand};
/// Call once with the actual pure State's current pending Copy request and command.
///
/// # Errors
/// Rejects stale, foreign, changed-session or non-increasing replacement bindings
/// before invoking the callback. Returned data remains inert correlation metadata.
pub fn with_pending_report_copy<T>(
    old: ReportBinding,
    current: ReportBinding,
    use_request: impl FnOnce(ReportRequestId, ReportCommand) -> T,
) -> Result<T, ReportSurfaceError> {
    let mut state =
        State::new(old, sugarsubstitute_contracts::incident::ReportRestartAvailability::Available);
    state.presented()?;
    state.preflight_replace(old, current)?;
    state.replaced(
        current,
        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
    );
    let request = state.request(current, Intent::Action(ReportAction::CopyText))?;
    Ok(use_request(request.id, ReportCommand::new(request.binding, ReportAction::CopyText)))
}
/// Exercise the real native request fence for a pending Restart without creating native resources.
/// Matching metadata grants no process, acknowledgement, transport or clipboard capability.
/// # Errors
/// Returns an invalid original/current view or request-phase rejection.
pub fn with_pending_report_restart<T>(
    old: sugarsubstitute_contracts::incident::ReportBinding,
    current: sugarsubstitute_contracts::incident::ReportBinding,
    observe: impl FnOnce(
        super::ReportRequestId,
        sugarsubstitute_contracts::incident::ReportCommand,
    ) -> T,
) -> Result<T, super::ReportSurfaceError> {
    use sugarsubstitute_contracts::incident::{
        ReportAction, ReportCommand, ReportRestartAvailability,
    };
    let mut state = State::new(old, ReportRestartAvailability::Available);
    state.presented()?;
    state.preflight_replace(old, current)?;
    state.replaced(current, ReportRestartAvailability::Available);
    let request = state.request(current, Intent::Action(ReportAction::DismissAndRequestRestart))?;
    Ok(observe(
        request.id,
        ReportCommand::new(request.binding, ReportAction::DismissAndRequestRestart),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, error::Error};
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
    #[test]
    fn actual_state_callback_supplies_current_copy_request_once() -> Result<(), Box<dyn Error>> {
        let current = binding(2, 1)?;
        let calls = Cell::new(0);
        with_pending_report_copy(binding(1, 1)?, current, |request, command| {
            calls.set(calls.get() + 1);
            assert_eq!(request.get(), 1);
            assert_eq!(command, ReportCommand::new(current, ReportAction::CopyText));
        })?;
        assert_eq!(calls.get(), 1);
        Ok(())
    }
    #[test]
    fn invalid_replacement_never_produces_request_or_invokes_callback() -> Result<(), Box<dyn Error>>
    {
        for current in [binding(1, 1)?, binding(2, 2)?] {
            let called = Cell::new(false);
            assert!(
                with_pending_report_copy(binding(1, 1)?, current, |_, _| called.set(true)).is_err()
            );
            assert!(!called.get());
        }
        Ok(())
    }
}
