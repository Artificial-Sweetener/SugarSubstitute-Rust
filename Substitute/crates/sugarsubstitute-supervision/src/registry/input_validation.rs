// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate every retained launch role before scope admission or native effects.
use super::*;

impl WindowsProcessRegistry {
    /// Borrow all role owners and reject conflicting or foreign inputs before transfer.
    pub(super) fn validate_input(
        &self,
        binding: FamilyBinding,
        original: &WindowsLaunchInput,
    ) -> Result<(), RegistryAdmissionError> {
        if let FamilyBinding::Preparation(owner) = binding {
            let probe = original.preparation.as_ref().and_then(|launch| launch.probe());
            // Only an ordinary preparation helper may carry its own update-attempt channel.
            let foreign_attempt = original.purpose.as_ref().is_some_and(|launch| {
                probe.is_some()
                    || launch.key.update_attempt().is_none_or(|channel| channel.owner() != owner)
                    || !launch.validates(launch.key)
            });
            if original.preparation.as_ref().is_none_or(|launch| launch.owner != owner)
                || probe.is_some_and(|probe| !probe.can_launch())
                || original.recovery_carrier.is_some()
                || original.profile_custody.is_some()
                || original.managed_custody.is_some()
                || original.backend_custody.is_some()
                || foreign_attempt
                || original.lifecycle.is_some()
                || original.output.is_some()
                || original.fixture_events.is_some()
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        } else if original.preparation.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if original.recovery_carrier.is_some()
            || original
                .profile_custody
                .as_ref()
                .is_some_and(|profile| profile.value.recovery_grant.is_some())
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(profile) = &original.profile_custody
            && (binding!=FamilyBinding::Desktop(profile.value.key.run()) || !profile.value.identity.same_as(&self.identity)
                || original.managed_custody.is_some() || original.backend_custody.is_some()
                || original.purpose.is_some() || profile.value.published_ack.as_ref().is_none_or(|ack|
                    ack.receipt()!=sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::ProfilePublished(profile.value.key))) {
                return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(source) = &original.managed_custody
            && binding != FamilyBinding::Managed(source.resource())
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let FamilyBinding::BackendCustody(key) = binding {
            if original.backend_custody.as_ref().is_none_or(|launch| !launch.validates(key))
                || original.purpose.is_some()
                || original.lifecycle.is_some()
                || original.output.is_some()
                || original.fixture_events.is_some()
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        } else if original.backend_custody.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let FamilyBinding::Diagnostic(key) = binding {
            if original
                .purpose
                .as_ref()
                .is_none_or(|launch| !launch.validates(PurposeKey::Diagnostic(key)))
                || original.lifecycle.is_some()
                || original.output.is_some()
                || original.fixture_events.is_some()
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        } else if original.purpose.is_some() && !matches!(binding, FamilyBinding::Preparation(_)) {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if binding.owner() != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        if original
            .output
            .as_ref()
            .is_some_and(|output| !output.validates(binding) || original.fixture_events.is_some())
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(launch) = &original.lifecycle
            && (binding != FamilyBinding::Desktop(launch.run)
                || original.fixture_events.is_some()
                || launch
                    .enrollment
                    .as_ref()
                    .is_none_or(|enrollment| !launch.bootstrap.matches_enrollment(enrollment)))
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        Ok(())
    }
}
