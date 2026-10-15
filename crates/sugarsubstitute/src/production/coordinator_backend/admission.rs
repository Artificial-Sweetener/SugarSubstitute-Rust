// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite preparation and real owned policy peaks before mutating lifecycle state.
use super::*;
use sugarsubstitute_application::supervision::ResourceStorageInput;

impl CoordinatorBackend {
    /// Preflight only fresh effects. An exact App-owned retry replay must return
    /// its original outcome after the resource has advanced to Retrying; it cannot
    /// require a new source/campaign or start a second native effect.
    pub(super) fn request_preparation(
        &self,
        binding: ClientBinding,
        request: &ResourceRequest,
        original: Option<ConnectionBudget>,
        input: Option<&BackendPreparation>,
        now: Moment,
    ) -> Result<Option<ResourceKey>> {
        let retry_replay = matches!(&request.command, ResourceCommand::RetryDead(_))
            && self.policy.request_replay(binding, request)?;
        if !retry_replay {
            Self::check_request_budget(&request.command, original, now)?;
        }
        if let ResourceCommand::Acquire(target) = &request.command
            && !self.records.iter().flatten().any(|r| r.target == *target && !r.settled)
            && input.is_none()
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        Ok(match &request.command {
            ResourceCommand::RetryDead(failed) if !retry_replay => {
                Some(self.retry_preparation(binding, *failed, input)?)
            }
            _ => None,
        })
    }

    /// Check inert timing separately from actual serving/native binding validation.
    pub(super) fn check_request_budget(
        command: &ResourceCommand,
        original: Option<ConnectionBudget>,
        now: Moment,
    ) -> Result<()> {
        match command {
            ResourceCommand::Acquire(target) => {
                let budget = original.ok_or(LifecycleRejection::WrongPhase)?;
                let expected = match target {
                    BackendTarget::Managed(_) => ConnectionBudgetKind::Managed,
                    BackendTarget::Attached(_) => ConnectionBudgetKind::Attached,
                };
                if budget.kind() != expected {
                    return Err(Box::new(LifecycleRejection::WrongPhase));
                }
                budget.probe_until(now)?;
                Ok(())
            }
            ResourceCommand::RetryDead(_) => {
                let budget = original.ok_or(LifecycleRejection::WrongPhase)?;
                if budget.kind() != ConnectionBudgetKind::Managed {
                    return Err(Box::new(LifecycleRejection::WrongPhase));
                }
                budget.probe_until(now)?;
                Ok(())
            }
            ResourceCommand::Release(_) | ResourceCommand::Reload(_) if original.is_none() => {
                Ok(())
            }
            _ => Err(Box::new(LifecycleRejection::WrongPhase)),
        }
    }

    /// Validate actual App-owner dynamic allocations and projected peak before
    /// mutation. The caller must use the matching input before borrowing
    /// `incident_policy` for enrollment; native causes retain their original funding.
    /// # Errors
    /// Overload rejects before mutation when actual claimed policy bytes cannot
    /// cover the owner-computed retained, projected and transient allocation bound.
    pub(crate) fn preflight_policy(&self, input: ResourceStorageInput<'_>) -> Result<()> {
        self.policy_admission(input)?;
        Ok(())
    }

    pub(super) fn policy_admission(
        &self,
        input: ResourceStorageInput<'_>,
    ) -> std::result::Result<(), LifecycleRejection> {
        self.policy_admission_for(input, self.policy_funding.bytes())
    }

    /// Constructor preflight uses only the closed candidate credit's checked quota.
    /// Normal calls derive the capacity from the funding owner above.
    pub(super) fn policy_admission_for(
        &self,
        input: ResourceStorageInput<'_>,
        capacity: u64,
    ) -> std::result::Result<(), LifecycleRejection> {
        let requirement = self.policy.storage_preflight(input)?;
        // App funds one returned transition and its operation/mandatory-outcome
        // scratch once. Host additionally clones only the returned transition into
        // undelivered storage and clones its effects while applying them. Neither
        // copy duplicates App scratch, private nodes or future policy liabilities.
        let extra = requirement
            .returned_transition_bytes()
            .checked_mul(2)
            .and_then(|bytes| {
                self.undelivered.as_ref().map_or(Some(bytes), |pending| {
                    Self::transition_storage(pending).and_then(|held| bytes.checked_add(held))
                })
            })
            .ok_or(LifecycleRejection::Overloaded)?;
        let available = capacity.checked_sub(extra).ok_or(LifecycleRejection::Overloaded)?;
        requirement.check_allocation_partition(available)
    }

    /// Count only public transition-owned allocations; private policy node sizing
    /// remains with the App owner. Opaque native causes keep their original guard.
    fn transition_storage(transition: &ResourceTransition) -> Option<u64> {
        let mut bytes = 0_u64;
        for (capacity, width) in [
            (transition.replies.capacity(), size_of::<ResourceReply>()),
            (transition.effects.capacity(), size_of::<ResourceEffect>()),
            (transition.invalidations.capacity(), size_of::<GrantedResourceInvalidation>()),
            (transition.replacements.capacity(), size_of::<GrantedResourceReplacement>()),
        ] {
            bytes = bytes.checked_add((capacity as u64).checked_mul(width as u64)?)?;
        }
        for effect in &transition.effects {
            if let ResourceEffect::Prepare { target: BackendTarget::Managed(target), .. } = effect {
                bytes = bytes.checked_add(target.allocated_bytes()? as u64)?;
            }
        }
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::super::recovery_tests::Fixture;
    use super::*;

    #[test]
    fn actual_result_copies_are_funded_without_tripling_application_scratch() -> Result<()> {
        let mut fixture = Fixture::new(true)?;
        let transition = fixture.exit()?;
        assert_eq!(transition.invalidations.len(), 2);
        let key = fixture.leases[0].resource;
        let backend = fixture.held.backend.as_mut();
        let requirement = backend.policy.storage_preflight(ResourceStorageInput::Current)?;
        let returned = requirement.returned_transition_bytes();
        assert!(returned > 0);
        assert!(requirement.transient_bytes() > returned);
        let pending = backend.undelivered.as_ref().ok_or("Original producer return absent")?;
        let held = CoordinatorBackend::transition_storage(pending)
            .ok_or("Actual public transition allocation overflow")?;
        assert!(held > 0);
        let required = requirement
            .allocation_bytes()
            .checked_add(returned.checked_mul(2).ok_or("Result-copy bound overflow")?)
            .and_then(|bytes| bytes.checked_add(held))
            .ok_or("Host partition overflow")?;
        // This fixture's actual original Execution claim already owns these bytes;
        // reducing its available quota cannot manufacture a larger funding lease.
        let original = backend.policy_funding.bytes();
        assert!(required <= original, "Actual policy fixture claim does not cover declared peak");
        let snapshot = backend.policy.snapshot();
        let pointer = backend
            .failure(key)
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<RootExit>())
            .map(std::ptr::from_ref)
            .ok_or("Original managed-exit cause absent")?;
        backend.policy_funding.limit_for_test(required)?;
        backend.policy_admission(ResourceStorageInput::Current)?;
        backend
            .policy_funding
            .limit_for_test(required.checked_sub(1).ok_or("Empty requirement")?)?;
        assert_eq!(
            backend.policy_admission(ResourceStorageInput::Current),
            Err(LifecycleRejection::Overloaded)
        );
        assert_eq!(backend.policy.snapshot(), snapshot);
        assert_eq!(
            backend
                .failure(key)
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<RootExit>())
                .map(std::ptr::from_ref),
            Some(pointer)
        );
        assert_eq!(
            backend.undelivered.as_ref().ok_or("Original return dropped")?.invalidations,
            transition.invalidations
        );
        backend.policy_funding.limit_for_test(original)?;
        Ok(())
    }
}
