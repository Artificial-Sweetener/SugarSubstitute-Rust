// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate each lifecycle request against the budget owned by its actual phase.
use super::{ParentError, ParentLifecycle, ParentResources, Result};
use sugarsubstitute_application::startup::FrontendBudget;
use sugarsubstitute_contracts::execution::{CounterDeadline, Moment};
use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError;
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

impl ParentLifecycle {
    /// Decode an ordinary request against its connection admission interval.
    pub(super) fn deadline(
        &self,
        header: WireHeader,
        resources: &ParentResources<'_, '_>,
    ) -> Result<Moment> {
        let until = resources.execution.client.decode_counter_deadline(CounterDeadline::new(
            header.deadline_qpc,
            self.original.frequency(),
        )?)?;
        let now = resources.execution.client.now()?;
        Self::bounded_deadline(until, now, resources)
    }

    /// Validate one decoded interval without sampling another native deadline.
    pub(super) fn bounded_deadline(
        until: Moment,
        now: Moment,
        resources: &ParentResources<'_, '_>,
    ) -> Result<Moment> {
        let kind = match resources.target {
            sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(_) => {
                sugarsubstitute_application::startup::ConnectionBudgetKind::Managed
            }
            sugarsubstitute_contracts::lifecycle::BackendTarget::Attached(_) => {
                sugarsubstitute_application::startup::ConnectionBudgetKind::Attached
            }
        };
        // This checks a supplied interval's bound. The accepted header/Moment are
        // cached once; this upper-bound calculation never replaces that cutoff.
        let maximum = sugarsubstitute_application::startup::ConnectionBudget::new(kind, now)?;
        if until > maximum.until() || now >= until {
            return Err(LifecyclePipeError::DeadlineExpired.into());
        }
        Ok(until)
    }

    /// Admit frontend work against the retained frontend cutoff, independent of connection mode.
    pub(super) fn frontend_deadline(
        &self,
        header: WireHeader,
        resources: &ParentResources<'_, '_>,
    ) -> Result<Moment> {
        let until = resources.execution.client.decode_counter_deadline(CounterDeadline::new(
            header.deadline_qpc,
            self.original.frequency(),
        )?)?;
        frontend_cutoff(
            until,
            resources.execution.client.now()?,
            self.main.snapshot().frontend_until.ok_or(ParentError::WrongPhase)?,
        )
    }
}

/// Preserve the earlier original cutoff; validation never restarts frontend preparation.
fn frontend_cutoff(supplied: Moment, now: Moment, issued: Moment) -> Result<Moment> {
    let maximum = FrontendBudget::new(now)?.until();
    let until = supplied.min(issued);
    if supplied > maximum || now >= until {
        return Err(LifecyclePipeError::DeadlineExpired.into());
    }
    Ok(until)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use sugarsubstitute_application::startup::{ConnectionBudget, ConnectionBudgetKind};

    #[test]
    fn attached_connection_does_not_shorten_the_separate_frontend_phase() -> Result<()> {
        let began = Moment::from_nanos(1_000_000_000);
        let connection = ConnectionBudget::new(ConnectionBudgetKind::Attached, began)?;
        let frontend = FrontendBudget::new(began)?;
        let received = began.checked_add(Duration::from_secs(10))?;
        assert!(frontend.until() > connection.until());
        assert_eq!(
            frontend_cutoff(frontend.until(), received, frontend.until())?,
            frontend.until()
        );
        Ok(())
    }

    #[test]
    fn frontend_validation_cannot_renew_expired_or_excessive_requests() -> Result<()> {
        let began = Moment::from_nanos(1_000_000_000);
        let parent = FrontendBudget::new(began)?.until();
        let received = began.checked_add(Duration::from_secs(10))?;
        let later_child = FrontendBudget::new(received)?.until();
        assert_eq!(frontend_cutoff(later_child, received, parent)?, parent);
        assert!(frontend_cutoff(received, received, parent).is_err());
        assert!(frontend_cutoff(later_child, parent, parent).is_err());
        let excessive = later_child.checked_add(Duration::from_nanos(1))?;
        assert!(frontend_cutoff(excessive, received, parent).is_err());
        Ok(())
    }
}
