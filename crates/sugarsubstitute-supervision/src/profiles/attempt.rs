// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Invalidate per-main authorization without releasing the run's physical profile custody.
use super::*;
use sugarsubstitute_contracts::{execution::Moment, startup::MainIdentity};

#[derive(Clone)]
/// Correlate exact presentation facts through a single bounded shared generation fence.
pub(super) struct Binding {
    pub(super) identity: MainIdentity,
    epoch: u64,
    fence: Arc<AtomicU64>,
}
impl Binding {
    /// Compare exact immutable identity and the still-active, never-reused epoch.
    pub(super) fn live(&self, identity: MainIdentity) -> bool {
        self.identity == identity && self.fence.load(Ordering::Acquire) == self.epoch
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
/// Require actual retirement admission before another presentation lifetime begins.
enum Phase {
    Active,
    Retiring,
    Retired,
}
/// Keep each proof submission and its earliest original deadline with one attempt.
struct Attempt {
    binding: Binding,
    phase: Phase,
    deadline: Option<Moment>,
    pub(super) folder_attempted: bool,
    pub(super) environment_attempted: bool,
    pub(super) browser_attempted: bool,
}
/// Reuse one atomic allocation while old proofs keep their immutable epoch values.
pub(super) struct Attempts {
    fence: Arc<AtomicU64>,
    current: Option<Attempt>,
    next: u64,
}
/// Select the distinct one-attempt authorization boundary without raw native authority.
pub(super) enum ProofKind {
    Folder,
    Environment,
    Browser,
}
impl Attempts {
    /// Create a lexical fence before any finite/native submission occurs.
    pub(super) fn new() -> Self {
        Self { fence: Arc::new(AtomicU64::new(0)), current: None, next: 1 }
    }
    /// Advance both presentation components after matching prior retirement.
    pub(super) fn begin(&mut self, identity: MainIdentity) -> Result<(), ClaimAdmissionError> {
        if let Some(old) = &self.current {
            if old.phase != Phase::Retired {
                return Err(ClaimAdmissionError::Busy);
            }
            if identity.launch.get() <= old.binding.identity.launch.get()
                || identity.navigation.get() <= old.binding.identity.navigation.get()
            {
                return Err(ClaimAdmissionError::Identity);
            }
        }
        let next = self.next.checked_add(1).ok_or(ClaimAdmissionError::Closed)?;
        let binding = Binding { identity, epoch: self.next, fence: self.fence.clone() };
        self.fence.store(self.next, Ordering::Release);
        self.next = next;
        self.current = Some(Attempt {
            binding,
            phase: Phase::Active,
            deadline: None,
            folder_attempted: false,
            environment_attempted: false,
            browser_attempted: false,
        });
        Ok(())
    }
    /// Borrow only a still-live exact attempt; retired proofs never reopen.
    pub(super) fn binding(&self, identity: MainIdentity) -> Result<Binding, ClaimAdmissionError> {
        let current = self.current.as_ref().ok_or(ClaimAdmissionError::Closed)?;
        if current.binding.identity != identity {
            return Err(ClaimAdmissionError::Identity);
        }
        if current.phase != Phase::Active {
            return Err(ClaimAdmissionError::Closed);
        }
        if !current.binding.live(identity) {
            return Err(ClaimAdmissionError::Closed);
        }
        Ok(current.binding.clone())
    }
    /// Latch each submission and the earliest deadline before queue admission.
    pub(super) fn admit(
        &mut self,
        identity: MainIdentity,
        deadline: Moment,
        operation: ProofKind,
    ) -> Result<Binding, ClaimAdmissionError> {
        let binding = self.binding(identity)?;
        let current = self.current.as_mut().ok_or(ClaimAdmissionError::Closed)?;
        if current.deadline.is_some_and(|old| deadline > old) {
            return Err(ClaimAdmissionError::Specification);
        }
        let attempted = match operation {
            ProofKind::Folder => &mut current.folder_attempted,
            ProofKind::Environment => &mut current.environment_attempted,
            ProofKind::Browser => &mut current.browser_attempted,
        };
        if *attempted {
            return Err(ClaimAdmissionError::Busy);
        }
        *attempted = true;
        current.deadline = Some(current.deadline.map_or(deadline, |old| old.min(deadline)));
        Ok(binding)
    }
    /// Invalidate before waiting for outstanding work or authenticating final retirement.
    pub(super) fn retire(&mut self, identity: MainIdentity) -> Result<(), ClaimAdmissionError> {
        let current = self.current.as_mut().ok_or(ClaimAdmissionError::Closed)?;
        if current.binding.identity != identity {
            return Err(ClaimAdmissionError::Identity);
        }
        self.fence.store(0, Ordering::Release);
        if current.phase == Phase::Active {
            current.phase = Phase::Retiring;
        }
        Ok(())
    }
    /// Accept the root's actual retirement after the public owner checks custody.
    pub(super) fn confirm(&mut self, identity: MainIdentity) -> Result<(), ClaimAdmissionError> {
        let current = self.current.as_mut().ok_or(ClaimAdmissionError::Closed)?;
        if current.binding.identity != identity {
            return Err(ClaimAdmissionError::Identity);
        }
        if current.phase == Phase::Active {
            return Err(ClaimAdmissionError::Busy);
        }
        current.phase = Phase::Retired;
        Ok(())
    }
    /// Permanently close every current proof before whole-profile release admission.
    pub(super) fn invalidate(&self) {
        self.fence.store(0, Ordering::Release);
    }
}
impl WindowsProfileLease {
    /// Begin an exact main attempt while retaining this run's exclusion and desktop family.
    /// # Errors
    /// Rejects foreign/nonincreasing identities, closing/uncertain custody or unconsumed jobs.
    pub fn begin_main_attempt(
        &mut self,
        identity: MainIdentity,
    ) -> Result<(), ClaimAdmissionError> {
        if identity.run != self.run {
            return Err(ClaimAdmissionError::Identity);
        }
        if self.pending.is_some() || !self.is_held() {
            return Err(ClaimAdmissionError::Busy);
        }
        self.attempts.begin(identity)?;
        self.folder = None;
        self.environment = None;
        self.browser = None;
        Ok(())
    }
    /// Close this attempt's authorization immediately, including while a finite job drains.
    /// # Errors
    /// Rejects absent/foreign attempts. Repeated retirement preserves the invalid fence.
    pub fn retire_main_attempt(
        &mut self,
        identity: MainIdentity,
    ) -> Result<(), ClaimAdmissionError> {
        self.attempts.retire(identity)
    }
    /// Admit the composing root's matching actual desktop `MainRetired` observation.
    /// Identities correlate evidence; the caller authenticates the observation at its desktop port.
    /// This method grants no native authority and cannot clear unresolved browser uncertainty.
    /// # Errors
    /// Rejects active/foreign attempts, an unconsumed operation, or uncertain browser custody.
    pub fn confirm_main_retired(
        &mut self,
        identity: MainIdentity,
    ) -> Result<(), ClaimAdmissionError> {
        if self.pending.is_some() || self.shared.active.load(Ordering::Acquire) == UNCERTAIN {
            return Err(ClaimAdmissionError::Busy);
        }
        self.attempts.confirm(identity)
    }
}
