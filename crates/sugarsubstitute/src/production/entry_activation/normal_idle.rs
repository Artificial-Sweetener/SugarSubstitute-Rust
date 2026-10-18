// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Propagate deliberate current-role close through original budgets and actual pending owners.
#[cfg(any(test, feature = "k1-native-qualification"))]
#[path = "normal_idle/qualification.rs"]
pub(super) mod qualification;
use super::*;
use crate::production::parent_lifecycle::DesktopCloseRequest;

/// Original role budget plus the existing sealed retirement producer's disposition.
enum Role {
    Normal { budget: ShutdownBudget, retired: bool },
    SettledFailedDesktop,
}
/// A denial belongs to at most one existing run or resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdleIdentity {
    Global,
    Run(RunKey),
    Resource(ResourceKey),
}
/// The first policy veto retains only existing owner identity and a closed diagnostic label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdleBlock {
    reason: &'static str,
    identity: IdleIdentity,
}
// Denials remain inline and below Clippy's large-result threshold on every target.
const _: () = assert!(std::mem::size_of::<IdleBlock>() < 128);
impl IdleBlock {
    /// Identify one global retained duty without inventing a run or resource.
    const fn global(reason: &'static str) -> Self {
        Self { reason, identity: IdleIdentity::Global }
    }
}
impl ProductionActivation {
    /// Borrow the original close budget and qualify only changes in the actual decision.
    pub(super) fn normal_close_budget(&self) -> Option<ShutdownBudget> {
        let decision = self.normal_close_decision();
        #[cfg(any(test, feature = "k1-native-qualification"))]
        self.observe_idle_gate(&decision);
        decision.ok()
    }
    /// Evaluate each existing owner veto once, preserving short-circuit order.
    fn normal_close_decision(&self) -> std::result::Result<ShutdownBudget, IdleBlock> {
        if self.closing {
            return Err(IdleBlock::global("Closing"));
        }
        if self.failure_source().is_some() {
            return Err(IdleBlock::global("FailureSource"));
        }
        if self.failure_run.is_some() {
            return Err(IdleBlock::global("FailureRun"));
        }
        if self.failure_resource.is_some() {
            return Err(IdleBlock::global("FailureResource"));
        }
        if let Some(reason) = self.incidents.idle_block() {
            return Err(IdleBlock::global(reason));
        }
        if self.pending.iter().any(Option::is_some) {
            return Err(IdleBlock::global("PendingProjection"));
        }
        if self.recovery_pending.as_ref().is_some_and(|pending| !pending.settled()) {
            return Err(IdleBlock::global("PendingRecovery"));
        }
        if self.delivery.as_ref().is_some_and(|delivery| delivery.pending()) {
            return Err(IdleBlock::global("PendingDelivery"));
        }
        let backend = &self.backend.as_ref().ok_or(IdleBlock::global("MissingBackend"))?.backend;
        if backend.delivery_pending() {
            return Err(IdleBlock::global("BackendDelivery"));
        }
        for key in backend.resource_keys().into_iter().flatten() {
            if backend.failure(key).is_some() || backend.cleanup_failure(key).is_some() {
                return Err(IdleBlock {
                    reason: "BackendFailure",
                    identity: IdleIdentity::Resource(key),
                });
            }
        }
        let capture = self.capture.as_ref().ok_or(IdleBlock::global("MissingCapture"))?;
        let mut blocked = None;
        let roles = self.records.iter().flatten().map(|held| {
            let record = &held.record;
            let decision = if capture.normal_close_pending(record.run.owner) {
                Err("CaptureClosePending")
            } else {
                recovery_retirement(
                    record.start.as_ref().is_none_or(DesktopStart::recovery_settled),
                )
                .and_then(|()| role_decision(record))
            };
            match decision {
                Ok(role) => Some(role),
                Err(reason) => {
                    blocked = Some(IdleBlock { reason, identity: IdleIdentity::Run(record.run) });
                    None
                }
            }
        });
        original_budget(roles, self.incidents.reporting.normal_close_budget())
            .ok_or(blocked.unwrap_or(IdleBlock::global("MissingAcceptedCloseIntent")))
    }
    /// Borrow only this accepted deliberate role's original grace-expired transaction.
    /// The exact launch ticket remains with existing native/profile/capture cleanup owners;
    /// other roles and backend interests remain serving. This is not normal-idle eligibility.
    pub(super) fn overdue_role_budget(
        record: &Record,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> Option<ShutdownBudget> {
        if record.closed || record.failed.is_some() || record.pending_desktop_failure.is_some() {
            return None;
        }
        let request = record.parent.as_ref()?.overdue_close_request(now)?;
        let start = record.start.as_ref()?;
        let ticket = start.ticket?;
        if request.run() != record.run
            || start.run != record.run
            || start.launch != record.launch
            || start.navigation != record.navigation
            || start.connection != record.connection
            || ticket.process() != request.process()
            || ticket.binding() != sugarsubstitute_supervision::FamilyBinding::Desktop(record.run)
        {
            return None;
        }
        Some(request.budget())
    }
    /// Borrow one eligible role for ordinary cleanup while another role may keep Root serving.
    pub(super) fn normal_role_budget(record: &Record) -> Option<ShutdownBudget> {
        match role_decision(record) {
            Ok(Role::Normal { budget, .. }) => Some(budget),
            Ok(Role::SettledFailedDesktop) | Err(_) => None,
        }
    }
}

/// Keep Root Entry live until its existing recovery control has genuinely retired.
/// Per-role cleanup remains eligible to send Stop and consume Retired before this global fence.
pub(in crate::production::entry_activation) fn recovery_retirement(
    retired: bool,
) -> std::result::Result<(), &'static str> {
    if retired { Ok(()) } else { Err("RecoveryRetirement") }
}
/// Classify the exact first veto at the authoritative role policy.
fn role_decision(record: &Record) -> std::result::Result<Role, &'static str> {
    if let Some(completed) = &record.completed_normal {
        return completed.budget(record).map(|budget| Role::Normal { budget, retired: true });
    }
    if record.collection.pending() {
        return Err("CollectionPending");
    }
    if !record.collection.delivered() {
        return Err("CollectionDelivery");
    }
    if record.pending_desktop_failure.is_some() {
        return Err("PendingDesktopFailure");
    }
    if record.seed.failure().is_some() {
        return Err("SeedFailure");
    }
    if record.attach.is_some() {
        return Err("Attachment");
    }
    if record.attach_failure.is_some() {
        return Err("AttachmentFailure");
    }
    if record.released_attach.is_some() {
        return Err("ReleasedAttachment");
    }
    if record.attach_retirement.is_some() {
        return Err("AttachmentRetirement");
    }
    if record.projected.is_some() {
        return Err("Projection");
    }
    if record.forwarded.is_some() {
        return Err("Forwarded");
    }
    if !record.replied {
        return Err("EntryReply");
    }
    if record.failed.is_some() {
        return settled_failed_role(record);
    }
    let parent = record.parent.as_ref().ok_or("MissingParent")?;
    if !parent.normal_cleanup_ready() {
        return Err("ParentNormalCleanup");
    }
    let request: DesktopCloseRequest =
        parent.normal_close_request().ok_or("MissingNormalCloseRequest")?;
    let start = record.start.as_ref().ok_or("MissingStart")?;
    if start.run != record.run
        || start.launch != record.launch
        || start.navigation != record.navigation
        || start.connection != record.connection
        || request.run() != record.run
    {
        return Err("NormalRoleIdentity");
    }
    if start.profile.snapshot().pending {
        return Err("ProfilePending");
    }
    if start.value().is_err() {
        return Err("StartFailure");
    }
    if start.output.as_ref().is_none_or(|output| output.normal_close_pending()) {
        return Err("NormalOutputPending");
    }
    let retired = match (&record.retired, start.ticket) {
        (Some(retired), None) if retired.run() == record.run && retired.registered() => {
            record.closed
        }
        (None, Some(ticket))
            if ticket.process() == request.process()
                && ticket.binding()
                    == sugarsubstitute_supervision::FamilyBinding::Desktop(record.run) =>
        {
            false
        }
        _ => return Err("NormalRetirementIdentity"),
    };
    Ok(Role::Normal { budget: request.budget(), retired })
}

/// Admit irreversible normal-role disposal only through every live-owner idle guard.
pub(super) fn completed_normal_budget(record: &Record) -> Option<ShutdownBudget> {
    if record.completed_normal.is_some()
        || !record.closed
        || record.failed_cause.is_some()
        || record.failed_shutdown.is_some()
        || record.failed_family_adopted
        || !record.seed.delivered()
        || (record.seed.started()
            && (!record.seeded || !matches!(record.seed.material(), Ok(Some(_)))))
        || record.start.as_ref().is_some_and(|start| start.recovery.is_some())
    {
        return None;
    }
    match role_decision(record) {
        Ok(Role::Normal { budget, retired: true }) => Some(budget),
        _ => None,
    }
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
        let Role::Normal { budget, retired } = role? else {
            continue;
        };
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
fn settled_failed_role(record: &Record) -> std::result::Result<Role, &'static str> {
    let fact = record.failed.ok_or("MissingFailedFact")?;
    let retired = record.retired.as_ref().ok_or("MissingRetiredDesktop")?;
    let parent = record.parent.as_ref().ok_or("MissingFailedParent")?;
    let start = record.start.as_ref().ok_or("MissingFailedStart")?;
    let expected_cause = match record.failed_cause.as_deref() {
        Some(cause) => {
            cause.downcast_ref::<crate::production::parent_lifecycle::ParentError>()
                == Some(&crate::production::parent_lifecycle::ParentError::DesktopStopped)
        }
        None => parent.failure().is_none(),
    };
    if !record.closed {
        return Err("RunPolicySettlement");
    }
    if !record.failed_family_adopted {
        return Err("FailedFamilyAdoption");
    }
    if !record.collection.completed_failed(fact) {
        return Err("FailedCollectionCompletion");
    }
    if !expected_cause {
        return Err("UnexpectedFailedCause");
    }
    if fact.run() != record.run {
        return Err("FailedFactIdentity");
    }
    if retired.run() != record.run {
        return Err("FailedRetirementRun");
    }
    if !retired.registered() {
        return Err("FailedRetirementRegistration");
    }
    if !parent.native_retired() {
        return Err("FailedParentNative");
    }
    if !parent.settled() {
        return Err("FailedParentSettlement");
    }
    if parent.failed_incident(record.run, fact.process()).map_err(|_| "FailedIncidentError")?
        != Some(fact)
    {
        return Err("FailedIncidentIdentity");
    }
    if start.run != record.run {
        return Err("FailedStartRun");
    }
    if start.launch != record.launch {
        return Err("FailedStartLaunch");
    }
    if start.navigation != record.navigation {
        return Err("FailedStartNavigation");
    }
    if start.connection != record.connection {
        return Err("FailedStartConnection");
    }
    if start.ticket.is_some() {
        return Err("FailedTicketRetained");
    }
    if start.profile.snapshot().pending {
        return Err("FailedProfilePending");
    }
    if !start.profile.snapshot().released {
        return Err("FailedProfileRelease");
    }
    if start.output.as_ref().is_some_and(|output| !output.settled()) {
        return Err("FailedOutputSettlement");
    }
    Ok(Role::SettledFailedDesktop)
}
#[cfg(test)]
#[path = "normal_idle_tests.rs"]
mod tests;
