// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded original projection credits through all policy/source destruction.
use super::*;
use crate::production::entry_activation::funding::ProjectionPolicyCredit;

/// Data-backed quota is never recycled by run retirement. This owner is the LAST
/// CoordinatorBackend field so all policy/history/source/return data dies first.
/// The original fixed claim separately funds this entire64-slot inline layout.
pub(super) struct PolicyFunding {
    credits: [Option<ProjectionPolicyCredit>; 64],
    bytes: u64,
}

impl PolicyFunding {
    pub(super) fn new() -> Self {
        Self { credits: std::array::from_fn(|_| None), bytes: 0 }
    }

    /// Bound the shared backend policy by the original4MiB role, independently of
    /// how many startup projections contribute real unused data-backed quotas.
    fn ceiling() -> Result<u64> {
        (4_u64 * 1024 * 1024)
            .checked_sub(RetainedBackend::required_storage_bytes())
            .ok_or_else(|| Box::new(LifecycleRejection::Overloaded) as Box<dyn Error + Send + Sync>)
    }

    pub(super) const fn bytes(&self) -> u64 {
        self.bytes
    }

    fn plan(&self, credit: &Option<ProjectionPolicyCredit>) -> Result<Option<(usize, u64)>> {
        let ceiling = Self::ceiling()?;
        let remaining = ceiling.checked_sub(self.bytes).ok_or(LifecycleRejection::Overloaded)?;
        // A65th lifetime cannot force a new slot when the global quota is full.
        // The still-owned unused credit remains with its projected data.
        if remaining == 0 {
            return Ok(None);
        }
        let Some(credit) = credit else {
            return Ok(None);
        };
        let contribution = credit.bytes().min(remaining);
        if contribution == 0 {
            return Ok(None);
        }
        let at =
            self.credits.iter().position(Option::is_none).ok_or(LifecycleRejection::Overloaded)?;
        let next = self
            .bytes
            .checked_add(contribution)
            .filter(|bytes| *bytes <= ceiling)
            .ok_or(LifecycleRejection::Overloaded)?;
        Ok(Some((at, next)))
    }

    /// Inspect the exact candidate's bounded contribution without taking it.
    pub(super) fn preview(&self, credit: &Option<ProjectionPolicyCredit>) -> Result<u64> {
        if credit.is_none() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let capacity = self.plan(credit)?.map_or(self.bytes, |(_, next)| next);
        if capacity == 0 {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        Ok(capacity)
    }

    /// Validate slot/ceiling/sum before taking the original nonclone credit.
    /// No fallible operation follows transfer. Saturation/zero unused quota leaves
    /// Some untouched, so unused bytes are never counted or prematurely released.
    pub(super) fn adopt(&mut self, credit: &mut Option<ProjectionPolicyCredit>) -> Result<()> {
        let Some((at, next)) = self.plan(credit)? else {
            return Ok(());
        };
        let original = credit.take().ok_or(LifecycleRejection::WrongPhase)?;
        self.credits[at] = Some(original);
        self.bytes = next;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn limit_for_test(&mut self, bytes: u64) -> Result<()> {
        let ceiling = Self::ceiling()?;
        let mut held = 0_u64;
        for credit in self.credits.iter().flatten() {
            held = held
                .checked_add(credit.bytes().min(ceiling - held))
                .ok_or(LifecycleRejection::Overloaded)?;
        }
        if bytes > held {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        self.bytes = bytes;
        Ok(())
    }
}

impl RetainedBackend {
    /// Return the fixed table plus original one-MiB witness metadata charge.
    /// The composing projection must separately claim actual policy allocation
    /// bytes within its unchanged total reservation, including every other owner.
    /// This minimum alone does not fund growing resource policy tables.
    pub(crate) const fn required_storage_bytes() -> u64 {
        (size_of::<CoordinatorBackend>() + 64 * size_of::<Option<Backend>>()) as u64
            + output::Subscribers::storage_bytes()
            + crate::production::coordinator_custody::WITNESS_STORAGE_BYTES
    }
    /// Construct only from an actually serving claims/registry association.
    /// The caller preclaims the table and heap header from its unchanged original
    /// projection reservation. Both allocations remain inseparable from that claim.
    /// The separate closed credit shares its original projection data claim.
    /// Fixed-storage surplus is never counted as policy quota. Every rejection
    /// before adoption preserves the caller's original credit in place.
    pub(crate) fn new(
        authority: &CoordinatorAuthority,
        storage: MemoryClaim,
        credit: &mut Option<crate::production::entry_activation::funding::ProjectionPolicyCredit>,
    ) -> Result<Self> {
        if !authority.serving() || authority.registry().is_none() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let owner = authority.key().ok_or(LifecycleRejection::ForeignOwner)?;
        if storage.bytes() < Self::required_storage_bytes() {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        let funding = PolicyFunding::new();
        let available = funding.preview(credit)?;
        let mut records = Vec::new();
        records.try_reserve_exact(64)?;
        records.resize_with(64, || None);
        let mut backend = Box::new(CoordinatorBackend {
            custody_options: None,
            custody: None,
            owner,
            policy: ResourceCoordinator::new(owner),
            records: records.into_boxed_slice(),
            output_clients: output::Subscribers::new()?,
            next_shutdown: 0,
            undelivered: None,
            policy_funding: funding,
        });
        backend.policy_admission_for(
            sugarsubstitute_application::supervision::ResourceStorageInput::Current,
            available,
        )?;
        backend.policy_funding.adopt(credit)?;
        Ok(Self { backend, _storage: storage })
    }
    /// Retain another original projection quota without creating a claim or pool.
    /// When the existing global ceiling is already funded, the caller keeps its
    /// unused credit. Slot/byte failures occur before any credit transfer.
    pub(crate) fn add_projection_credit(
        &mut self,
        credit: &mut Option<crate::production::entry_activation::funding::ProjectionPolicyCredit>,
    ) -> Result<()> {
        self.backend.policy_funding.adopt(credit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credit(bytes: u64) -> Result<Option<ProjectionPolicyCredit>> {
        Ok(Some(crate::production::entry_activation::funding::test_policy_credit(
            crate::production::coordinator_backend::tests::storage_tests::claim(bytes)?,
        )?))
    }

    #[test]
    fn real_credit_adoption_is_once_and_fixed_storage_is_not_policy_quota() -> Result<()> {
        let mut owner = PolicyFunding::new();
        let mut input = credit(64 * 1024)?;
        let bytes = input.as_ref().ok_or("Actual closed credit absent")?.bytes();
        assert_eq!(owner.bytes(), 0);
        assert_eq!(owner.preview(&input)?, bytes);
        owner.adopt(&mut input)?;
        assert!(input.is_none());
        assert_eq!(owner.bytes(), bytes);
        owner.adopt(&mut input)?;
        assert_eq!(owner.bytes(), bytes);
        assert_eq!(owner.credits.iter().flatten().count(), 1);
        Ok(())
    }

    #[test]
    fn full_global_quota_keeps_surplus_credit_without_a_new_slot() -> Result<()> {
        let mut owner = PolicyFunding::new();
        let mut initial = credit(4 * 1024 * 1024)?;
        owner.adopt(&mut initial)?;
        assert!(initial.is_none());
        assert_eq!(owner.bytes(), PolicyFunding::ceiling()?);
        let mut surplus = credit(4096)?;
        let bytes = surplus.as_ref().ok_or("Surplus original absent")?.bytes();
        owner.adopt(&mut surplus)?;
        assert_eq!(surplus.as_ref().ok_or("Uncounted original was taken")?.bytes(), bytes);
        assert_eq!(owner.credits.iter().flatten().count(), 1);
        assert_eq!(owner.bytes(), PolicyFunding::ceiling()?);
        Ok(())
    }

    #[test]
    fn sixty_four_retained_contributions_reject_new_credit_before_taking_it() -> Result<()> {
        let mut owner = PolicyFunding::new();
        for _ in 0..64 {
            let mut original = credit(512)?;
            owner.adopt(&mut original)?;
            assert!(original.is_none());
        }
        assert!(owner.bytes() < PolicyFunding::ceiling()?);
        let before = owner.bytes();
        let mut rejected = credit(512)?;
        let bytes = rejected.as_ref().ok_or("Rejected original absent")?.bytes();
        let cause = owner.adopt(&mut rejected).err().ok_or("65th credit was accepted")?;
        assert_eq!(
            cause.downcast_ref::<LifecycleRejection>(),
            Some(&LifecycleRejection::Overloaded)
        );
        assert_eq!(rejected.as_ref().ok_or("Rejected credit dropped")?.bytes(), bytes);
        assert_eq!(owner.bytes(), before);
        assert_eq!(owner.credits.iter().flatten().count(), 64);
        Ok(())
    }

    #[test]
    fn small_witness_configuration_fits_original_role_without_starting_a_helper() -> Result<()> {
        let mut retained = super::super::tests::unadmitted_policy()?;
        let backend = retained.backend.as_mut();
        backend.configure_custody(
            r"D:\SugarSubstitute-Rust\artifacts\unopened-helper.exe".into(),
            r"D:\SugarSubstitute-Rust\artifacts\unopened-installation".into(),
        )?;
        assert!(backend.custody_options.is_some());
        assert!(backend.custody.is_none());
        assert!(backend.records.iter().all(Option::is_none));
        Ok(())
    }

    #[test]
    fn oversized_actual_witness_path_capacity_rejects_before_retaining_or_native_effects()
    -> Result<()> {
        let mut retained = super::super::tests::unadmitted_policy()?;
        let backend = retained.backend.as_mut();
        let mut executable = std::path::PathBuf::with_capacity(
            crate::production::coordinator_custody::WITNESS_STORAGE_BYTES as usize + 1,
        );
        executable.push(r"D:\SugarSubstitute-Rust\artifacts\unopened-helper.exe");
        let error = backend
            .configure_custody(
                executable,
                r"D:\SugarSubstitute-Rust\artifacts\unopened-installation".into(),
            )
            .err()
            .ok_or("Unfunded witness path was retained")?;
        assert_eq!(
            error.downcast_ref::<LifecycleRejection>(),
            Some(&LifecycleRejection::Overloaded)
        );
        assert!(backend.custody_options.is_none());
        assert!(backend.custody.is_none());
        assert!(backend.records.iter().all(Option::is_none));
        Ok(())
    }
}
