// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Propagate deliberate current-role close through original budgets and actual pending owners.
use super::*;
use crate::production::parent_lifecycle::NormalCloseRequest;

/// Original role budget plus the existing sealed retirement producer's disposition.
enum Role {
    Normal { budget: ShutdownBudget, retired: bool },
    SettledFailedDesktop,
}
impl ProductionActivation {
    /// Borrow actual current custody; the existing close owners prove all settlement after this request.
    pub(super) fn normal_close_budget(&self) -> Option<ShutdownBudget> {
        if self.closing
            || self.failure_source().is_some()
            || self.failure_run.is_some()
            || self.failure_resource.is_some()
            || self.incidents.diagnostic_custody_pending()
            || self.pending.iter().any(Option::is_some)
            || self.delivery.as_ref().is_some_and(|delivery| delivery.pending())
        {
            return None;
        }
        let backend = &self.backend.as_ref()?.backend;
        if backend.delivery_pending()
            || backend
                .resource_keys()
                .into_iter()
                .flatten()
                .any(|key| backend.failure(key).is_some() || backend.cleanup_failure(key).is_some())
        {
            return None;
        }
        let capture = self.capture.as_ref()?;
        let roles = self.records.iter().flatten().map(|held| {
            if capture.normal_close_pending(held.record.run.owner) {
                return None;
            }
            role(&held.record)
        });
        original_budget(roles, self.incidents.reporting.normal_close_budget())
    }
    /// Borrow one eligible role for ordinary cleanup while another role may keep Root serving.
    pub(super) fn normal_role_budget(record: &Record) -> Option<ShutdownBudget> {
        match role(record)? {
            Role::Normal { budget, .. } => Some(budget),
            Role::SettledFailedDesktop => None,
        }
    }
}

fn role(record: &Record) -> Option<Role> {
    if record.collection.pending()
        || !record.collection.delivered()
        || record.pending_desktop_failure.is_some()
        || record.seed.failure().is_some()
        || record.attach.is_some()
        || record.attach_failure.is_some()
        || record.released_attach.is_some()
        || record.attach_retirement.is_some()
        || record.projected.is_some()
        || record.forwarded.is_some()
        || !record.replied
    {
        return None;
    }
    if record.failed.is_some() {
        return settled_failed_role(record);
    }
    let parent = record.parent.as_ref()?;
    if !parent.normal_cleanup_ready() {
        return None;
    }
    let request: NormalCloseRequest = parent.normal_close_request()?;
    let start = record.start.as_ref()?;
    if start.run != record.run
        || start.launch != record.launch
        || start.navigation != record.navigation
        || start.connection != record.connection
        || request.run() != record.run
        || start.profile.snapshot().pending
        || start.value().is_err()
        || start.output.as_ref().is_none_or(|output| output.normal_close_pending())
    {
        return None;
    }
    let retired = match (&record.retired, start.ticket) {
        (Some(retired), None) if retired.run() == record.run && retired.registered() => {
            // Retain the original unresolved cutoff until run-local policy settlement.
            record.closed
        }
        (None, Some(ticket))
            if ticket.process() == request.process()
                && ticket.binding()
                    == sugarsubstitute_supervision::FamilyBinding::Desktop(record.run) =>
        {
            false
        }
        _ => return None,
    };
    Some(Role::Normal { budget: request.budget(), retired })
}
/// All roles must be eligible. Earliest unresolved transactions remain binding;
/// genuinely completed historical transactions use only the latest original close intent.
fn original_budget(
    roles: impl Iterator<Item = Option<Role>>,
    reporter: Option<ShutdownBudget>,
) -> Option<ShutdownBudget> {
    let mut unresolved = None;
    let mut completed = reporter;
    for role in roles {
        let Role::Normal { budget, retired } = role? else { continue };
        if retired {
            completed = Some(completed.map_or(budget, |old: ShutdownBudget| {
                if old.started < budget.started { budget } else { old }
            }));
        } else {
            unresolved = Some(unresolved.map_or(budget, |old: ShutdownBudget| ShutdownBudget {
                started: old.started.min(budget.started),
                graceful_until: old.graceful_until.min(budget.graceful_until),
                settle_until: old.settle_until.min(budget.settle_until),
            }));
        }
    }
    unresolved.or(completed)
}
/// Historical failure never supplies a new close intent or replaces an unknown fatal cause.
/// The actual Parent exit, native/registry/profile retirement and accepted incident family
/// transition must agree before its completed old budget stops constraining another role.
fn settled_failed_role(record: &Record) -> Option<Role> {
    let fact = record.failed?;
    let retired = record.retired.as_ref()?;
    let parent = record.parent.as_ref()?;
    let start = record.start.as_ref()?;
    let expected_cause = match record.failed_cause.as_deref() {
        Some(cause) => {
            cause.downcast_ref::<crate::production::parent_lifecycle::ParentError>()
                == Some(&crate::production::parent_lifecycle::ParentError::DesktopStopped)
        }
        None => parent.failure().is_none(),
    };
    if !record.closed
        || !record.failed_family_adopted
        || !record.collection.completed_failed(fact)
        || !expected_cause
        || fact.run() != record.run
        || retired.run() != record.run
        || !retired.registered()
        || !parent.native_retired()
        || !parent.settled()
        || parent.failed_incident(record.run, fact.process()).ok()? != Some(fact)
        || start.run != record.run
        || start.launch != record.launch
        || start.navigation != record.navigation
        || start.connection != record.connection
        || start.ticket.is_some()
        || start.profile.snapshot().pending
        || !start.profile.snapshot().released
        || start.output.as_ref().is_some_and(|output| !output.settled())
    {
        return None;
    }
    Some(Role::SettledFailedDesktop)
}
#[cfg(test)]
#[path = "normal_idle_tests.rs"]
mod tests;
