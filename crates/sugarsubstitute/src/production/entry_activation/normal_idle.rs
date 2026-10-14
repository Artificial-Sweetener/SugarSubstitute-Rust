// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Propagate deliberate current-role close through original budgets and actual pending owners.
use super::*;
use crate::production::parent_lifecycle::NormalCloseRequest;

/// Original role budget plus the existing sealed retirement producer's disposition.
struct Role {
    budget: ShutdownBudget,
    retired: bool,
}
impl ProductionActivation {
    /// Borrow actual current custody; the existing close owners prove all settlement after this request.
    pub(super) fn normal_close_budget(&self) -> Option<ShutdownBudget> {
        if self.closing
            || self.failure_source().is_some()
            || self.failure_run.is_some()
            || self.failure_resource.is_some()
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
        original_budget(roles)
    }
    /// Borrow one eligible role for ordinary cleanup while another role may keep Root serving.
    pub(super) fn normal_role_budget(record: &Record) -> Option<ShutdownBudget> {
        role(record).map(|role| role.budget)
    }
}

fn role(record: &Record) -> Option<Role> {
    if record.attach.is_some()
        || record.attach_failure.is_some()
        || record.released_attach.is_some()
        || record.attach_retirement.is_some()
        || record.projected.is_some()
        || record.forwarded.is_some()
        || !record.replied
    {
        return None;
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
        (Some(retired), None) if retired.run() == record.run && retired.registered() => true,
        (None, Some(ticket))
            if ticket.process() == request.process()
                && ticket.binding()
                    == sugarsubstitute_supervision::FamilyBinding::Desktop(record.run) =>
        {
            false
        }
        _ => return None,
    };
    Some(Role { budget: request.budget(), retired })
}
/// All roles must be eligible. Earliest unresolved transactions remain binding;
/// genuinely completed historical transactions use only the latest original close intent.
fn original_budget(roles: impl Iterator<Item = Option<Role>>) -> Option<ShutdownBudget> {
    let mut unresolved = None;
    let mut completed = None;
    for role in roles {
        let role = role?;
        if role.retired {
            completed = Some(completed.map_or(role.budget, |old: ShutdownBudget| {
                if old.started < role.budget.started { role.budget } else { old }
            }));
        } else {
            unresolved =
                Some(unresolved.map_or(role.budget, |old: ShutdownBudget| ShutdownBudget {
                    started: old.started.min(role.budget.started),
                    graceful_until: old.graceful_until.min(role.budget.graceful_until),
                    settle_until: old.settle_until.min(role.budget.settle_until),
                }));
        }
    }
    unresolved.or(completed)
}
#[cfg(test)]
#[path = "normal_idle_tests.rs"]
mod tests;
