// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one admitted fresh supervisor without taking coordinator, backend or profile custody.
//!
//! The adapter supplies actual admission, durable storage, peer, family and native
//! retirement observations. Copied keys and this policy's phase grant no native
//! privilege. The original Root remains the only backend/profile policy owner.
mod state;
#[cfg(test)]
mod tests;
pub use state::*;
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::Moment,
    incident::DurableAcknowledgement,
    lifecycle::{RunKey, ShutdownBudget},
    recovery_supervisor::*,
};
/// Serialize one restart reservation and independently observed original/fresh custody facts.
pub struct RecoverySupervisor {
    key: RecoveryGrantKey,
    original: EntrySetupBudget,
    phase: RecoverySupervisorPhase,
    acknowledgement: Option<DurableAcknowledgement>,
    old_family_retired: bool,
    old_profile_retired: bool,
    supervisor: Option<RecoveryProcessIdentity>,
    handler: Option<RecoveryProcessIdentity>,
    desktop: Option<RecoveryProcessIdentity>,
    coverage: RecoveryCaptureCoverage,
    effect: Option<RecoverySupervisorEffect>,
    first_failure: Option<RecoveryFailureCode>,
    shutdown: Option<ShutdownBudget>,
    retirement: u8,
}
impl RecoverySupervisor {
    /// Retain an inert original preparation interval. Construction admits nothing.
    pub const fn new(key: RecoveryGrantKey, original: EntrySetupBudget) -> Self {
        Self {
            key,
            original,
            phase: RecoverySupervisorPhase::Unadmitted,
            acknowledgement: None,
            old_family_retired: false,
            old_profile_retired: false,
            supervisor: None,
            handler: None,
            desktop: None,
            coverage: RecoveryCaptureCoverage::PriorRegistrationsLost,
            effect: None,
            first_failure: None,
            shutdown: None,
            retirement: 0,
        }
    }
    /// Observe a held genuine execution reservation before permitting durable Restart ACK.
    /// # Errors
    /// Foreign/repeated/expired admission leaves all existing policy facts unchanged.
    pub fn admitted(
        &mut self,
        key: RecoveryGrantKey,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if self.phase != RecoverySupervisorPhase::Unadmitted {
            return Err(RecoveryRejection::Phase);
        }
        self.phase = RecoverySupervisorPhase::Reserved;
        Ok(())
    }
    /// Recheck current reserved custody immediately before the adapter issues durable ACK.
    /// The adapter must additionally retain and validate the opaque execution reservation.
    pub fn acknowledgement_allowed(&self, key: RecoveryGrantKey, now: Moment) -> bool {
        self.check(key).is_ok()
            && self.current(now).is_ok()
            && self.phase == RecoverySupervisorPhase::Reserved
            && self.first_failure.is_none()
    }
    /// Adopt the actual committed acknowledgement for this exact view and session.
    /// # Errors
    /// Premature, foreign or changed commits reject before launch or revision mutation.
    pub fn acknowledged(
        &mut self,
        key: RecoveryGrantKey,
        ack: DurableAcknowledgement,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if ack.incident() != key.report().incident()
            || key.report().revision().next().map_err(|_| RecoveryRejection::Binding)?
                != ack.revision()
        {
            return Err(RecoveryRejection::Binding);
        }
        if let Some(original) = self.acknowledgement {
            return if original == ack { Ok(()) } else { Err(RecoveryRejection::Binding) };
        }
        if self.phase != RecoverySupervisorPhase::Reserved {
            return Err(RecoveryRejection::Phase);
        }
        self.acknowledgement = Some(ack);
        self.maybe_launch();
        Ok(())
    }
    /// Observe exact original family retirement; profile release remains independent.
    /// # Errors
    /// Foreign identity or closed admission leaves both obligations unchanged.
    pub fn original_family_retired(&mut self, run: RunKey) -> Result<(), RecoveryRejection> {
        self.original_retirement(run)?;
        self.old_family_retired = true;
        self.maybe_launch();
        Ok(())
    }
    /// Observe actual original profile release; family exit alone cannot supply this fact.
    /// # Errors
    /// Foreign identity or closed admission leaves both obligations unchanged.
    pub fn original_profile_retired(&mut self, run: RunKey) -> Result<(), RecoveryRejection> {
        self.original_retirement(run)?;
        self.old_profile_retired = true;
        self.maybe_launch();
        Ok(())
    }
    /// Consume a bounded policy effect once. Actual admission and terminal observation stay external.
    pub fn take_effect(&mut self) -> Option<RecoverySupervisorEffect> {
        self.effect.take()
    }
    /// Observe actual once-only creation admission after consuming the launch effect.
    /// # Errors
    /// An expired original interval, occupied effect or premature/repeated issue rejects.
    pub fn launch_issued(
        &mut self,
        key: RecoveryGrantKey,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if self.phase != RecoverySupervisorPhase::LaunchReady || self.effect.is_some() {
            return Err(RecoveryRejection::Phase);
        }
        self.phase = RecoverySupervisorPhase::LaunchIssued;
        Ok(())
    }
    /// Adopt actual kernel/capability authentication of the launched subordinate.
    /// # Errors
    /// Changed grant, repeated peer or expired enrollment leaves the original owner unchanged.
    pub fn authenticated(
        &mut self,
        key: RecoveryGrantKey,
        process: RecoveryProcessIdentity,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if self.phase != RecoverySupervisorPhase::LaunchIssued {
            return Err(RecoveryRejection::Phase);
        }
        self.supervisor = Some(process);
        self.phase = RecoverySupervisorPhase::Authenticated;
        Ok(())
    }
    /// Observe genuine fresh registration and independently watched matching handler.
    /// This does not repair the old Root or previously registered live Desktops.
    /// # Errors
    /// Foreign supervisor, same-process handler or premature/expired registration rejects.
    pub fn capture_registered(
        &mut self,
        key: RecoveryGrantKey,
        supervisor: RecoveryProcessIdentity,
        handler: RecoveryProcessIdentity,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if self.phase != RecoverySupervisorPhase::Authenticated
            || self.supervisor != Some(supervisor)
            || supervisor.pid() == handler.pid()
        {
            return Err(RecoveryRejection::Binding);
        }
        self.handler = Some(handler);
        self.coverage = RecoveryCaptureCoverage::FreshSupervisorRegistered { supervisor, handler };
        self.phase = RecoverySupervisorPhase::Registered;
        Ok(())
    }
    /// Observe the real Desktop member after actual creation and its genuine capture registration.
    /// The verified member remains distinct from its supervisor carrier and handler.
    /// # Errors
    /// Same-process substitution, changed grant and premature/expired creation reject.
    pub fn desktop_capture_registered(
        &mut self,
        key: RecoveryGrantKey,
        process: RecoveryProcessIdentity,
        now: Moment,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        self.current(now)?;
        if self.phase != RecoverySupervisorPhase::Registered
            || self.supervisor.is_some_and(|own| own.pid() == process.pid())
            || self.handler.is_some_and(|own| own.pid() == process.pid())
        {
            return Err(RecoveryRejection::Binding);
        }
        let supervisor = self.supervisor.ok_or(RecoveryRejection::Binding)?;
        let handler = self.handler.ok_or(RecoveryRejection::Binding)?;
        self.coverage = RecoveryCaptureCoverage::FreshFamilyRegistered { supervisor, handler };
        self.desktop = Some(process);
        self.phase = RecoverySupervisorPhase::Active;
        Ok(())
    }
    /// Latch the first actual failure and request one original checked shutdown transaction.
    /// No cancellation or timeout is interpreted as process or handle retirement.
    pub fn fail(&mut self, reason: RecoveryFailureCode) {
        if self.phase == RecoverySupervisorPhase::Retired {
            return;
        }
        self.first_failure.get_or_insert(reason);
        self.stop();
    }
    /// Request normal shutdown without inventing a failure or replacing a pending close interval.
    /// The same complete physical retirement observations remain mandatory.
    pub fn stop(&mut self) {
        if !matches!(
            self.phase,
            RecoverySupervisorPhase::Retiring | RecoverySupervisorPhase::Retired
        ) {
            self.effect = Some(RecoverySupervisorEffect::BeginShutdown { key: self.key });
            self.phase = RecoverySupervisorPhase::Retiring;
        }
    }
    /// Observe deadline expiry using the same original setup or first admitted shutdown interval.
    pub fn observe_time(&mut self, now: Moment) {
        let original_expired = !matches!(
            self.phase,
            RecoverySupervisorPhase::Active
                | RecoverySupervisorPhase::Retiring
                | RecoverySupervisorPhase::Retired
        ) && (now < self.original.started() || now >= self.original.until());
        let shutdown_expired = self.shutdown.is_some_and(|budget| now >= budget.settle_until);
        if original_expired || shutdown_expired {
            self.fail(RecoveryFailureCode::Deadline);
        }
    }
    /// Retain the first genuine owner-supplied shutdown budget; it cannot be renewed.
    /// # Errors
    /// A changed budget or any phase without pending actual shutdown rejects.
    pub fn shutdown_admitted(&mut self, budget: ShutdownBudget) -> Result<(), RecoveryRejection> {
        if self.phase != RecoverySupervisorPhase::Retiring || self.effect.is_some() {
            return Err(RecoveryRejection::Phase);
        }
        if self.shutdown.is_some_and(|old| old != budget) {
            return Err(RecoveryRejection::Deadline);
        }
        self.shutdown = Some(budget);
        Ok(())
    }
    /// Observe one independent actual retirement. Every admitted physical owner must be proven.
    /// # Errors
    /// Foreign grant, missing shutdown admission and duplicate proof categories reject.
    pub fn retired(
        &mut self,
        key: RecoveryGrantKey,
        proof: RecoveryRetirement,
    ) -> Result<(), RecoveryRejection> {
        self.check(key)?;
        if self.phase != RecoverySupervisorPhase::Retiring || self.shutdown.is_none() {
            return Err(RecoveryRejection::Phase);
        }
        let bit = proof.bit();
        if self.retirement & bit != 0 {
            return Err(RecoveryRejection::Phase);
        }
        self.retirement |= bit;
        if self.retirement == 15 {
            self.phase = RecoverySupervisorPhase::Retired;
        }
        Ok(())
    }
    /// Borrow immutable policy observations, independent of native capabilities.
    pub fn snapshot(&self) -> RecoverySupervisorSnapshot {
        RecoverySupervisorSnapshot {
            key: self.key,
            phase: self.phase,
            coverage: self.coverage,
            first_failure: self.first_failure,
            supervisor: self.supervisor,
            desktop: self.desktop,
        }
    }
    fn check(&self, key: RecoveryGrantKey) -> Result<(), RecoveryRejection> {
        if key == self.key { Ok(()) } else { Err(RecoveryRejection::Binding) }
    }
    fn current(&self, now: Moment) -> Result<(), RecoveryRejection> {
        if now < self.original.started() || now >= self.original.until() {
            Err(RecoveryRejection::Deadline)
        } else {
            Ok(())
        }
    }
    fn original_retirement(&self, run: RunKey) -> Result<(), RecoveryRejection> {
        if run != self.key.previous() {
            return Err(RecoveryRejection::Binding);
        }
        if !matches!(
            self.phase,
            RecoverySupervisorPhase::Unadmitted | RecoverySupervisorPhase::Reserved
        ) {
            return Err(RecoveryRejection::Phase);
        }
        Ok(())
    }
    fn maybe_launch(&mut self) {
        if self.phase == RecoverySupervisorPhase::Reserved
            && self.acknowledgement.is_some()
            && self.old_family_retired
            && self.old_profile_retired
        {
            self.effect =
                Some(RecoverySupervisorEffect::Launch { key: self.key, original: self.original });
            self.phase = RecoverySupervisorPhase::LaunchReady;
        }
    }
}
