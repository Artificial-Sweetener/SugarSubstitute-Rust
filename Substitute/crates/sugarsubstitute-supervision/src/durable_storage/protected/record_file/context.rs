// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original execution capabilities through publication and checked cleanup.
use crate::ClaimError;
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// Retain original cancellation, clock and memory even after the task returns.
pub(super) struct PublicationContext {
    original: JobContext,
    custody: Option<crate::native_claims::files::Budget>,
    deadline: Moment,
    cleanup: Option<CleanupContext>,
}
struct CleanupContext {
    original: JobContext,
    deadline: Moment,
}
impl PublicationContext {
    /// Bind before any record read or stage may return a retained output.
    pub(super) fn new(
        context: &JobContext,
        budget: &crate::native_claims::files::Budget,
    ) -> Result<Self, ClaimError> {
        if matches!(budget, crate::native_claims::files::Budget::Custody(_)) {
            budget.check()?;
            return Ok(Self {
                original: context.clone(),
                custody: Some(budget.clone()),
                deadline: Moment::from_nanos(u64::MAX),
                cleanup: None,
            });
        }
        let deadline = live(context)?;
        Ok(Self { original: context.clone(), custody: None, deadline, cleanup: None })
    }
    /// Tighten retry deadlines without replacing original capability ports.
    pub(super) fn validate(&mut self, current: &JobContext) -> Result<(), ClaimError> {
        if let Some(original) = &self.custody {
            return original.check();
        }
        if self.cleanup.is_some() {
            return Err(ClaimError::invalid("publication_retirement_started"));
        }
        let next = current.deadline().ok_or_else(|| ClaimError::invalid("publication_deadline"))?;
        if current.owner_id() != self.original.owner_id()
            || current.scope_id() != self.original.scope_id()
            || next > self.deadline
        {
            return Err(ClaimError::invalid("changed_publication_context"));
        }
        self.deadline = self.deadline.min(next);
        self.check_original()?;
        live(current)?;
        Ok(())
    }
    /// Check retained original ports around inner native write primitives.
    pub(super) fn check_original(&self) -> Result<(), ClaimError> {
        if let Some(original) = &self.custody {
            return original.check();
        }
        if self.original.cancellation_requested() || self.original.now() >= self.deadline {
            Err(ClaimError::invalid("publication_context_expired"))
        } else {
            Ok(())
        }
    }
    /// Latch a separate bounded cleanup lifetime within the original executor.
    pub(super) fn validate_cleanup(&mut self, current: &JobContext) -> Result<(), ClaimError> {
        if let Some(original) = &self.custody {
            return original.check();
        }
        if current.owner_id() != self.original.owner_id() {
            return Err(ClaimError::invalid("foreign_publication_cleanup"));
        }
        let deadline = live(current)?;
        if let Some(cleanup) = self.cleanup.as_mut() {
            if current.scope_id() != cleanup.original.scope_id() || deadline > cleanup.deadline {
                return Err(ClaimError::invalid("renewed_publication_cleanup"));
            }
            cleanup.deadline = cleanup.deadline.min(deadline);
            if cleanup.original.cancellation_requested()
                || cleanup.original.now() >= cleanup.deadline
            {
                return Err(ClaimError::invalid("publication_cleanup_expired"));
            }
        } else {
            let now = current.now().as_nanos();
            let limit = now
                .checked_add(15_000_000_000)
                .ok_or_else(|| ClaimError::invalid("publication_cleanup_bound"))?;
            if deadline.as_nanos() > limit {
                return Err(ClaimError::invalid("publication_cleanup_bound"));
            }
            self.cleanup = Some(CleanupContext { original: current.clone(), deadline });
        }
        Ok(())
    }
}
/// Validate actual current admission without manufacturing an absent deadline.
pub(super) fn live(context: &JobContext) -> Result<Moment, ClaimError> {
    let deadline = context.deadline().ok_or_else(|| ClaimError::invalid("publication_deadline"))?;
    if context.cancellation_requested() || context.now() >= deadline {
        Err(ClaimError::invalid("publication_context_expired"))
    } else {
        Ok(deadline)
    }
}
