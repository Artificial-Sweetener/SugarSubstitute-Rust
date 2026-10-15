// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share one existing projection claim without returning credit while its data lives.
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::MemoryClaim, lifecycle::LifecycleRejection};

struct Backing {
    _storage: MemoryClaim,
}

/// Retain projection data independently of its single policy-credit recipient.
/// No raw claim, weak reference or naked allocation owner escapes this boundary.
pub(super) struct ProjectionDataFunding {
    backing: Option<Arc<Backing>>,
}
/// Retain only the unused quota of the SAME original projection data claim.
/// The backend owns this nonclone value until all policy and returned data die.
/// This allocation guard grants no source, process or native settlement authority.
pub(in crate::production) struct ProjectionPolicyCredit {
    backing: Option<Arc<Backing>>,
    bytes: u64,
}
impl ProjectionPolicyCredit {
    /// Return only the checked unused quota; never the whole reservation.
    pub(in crate::production) const fn bytes(&self) -> u64 {
        self.bytes
    }
}
/// Actual retained buffers plus explicitly named future copies from those buffers.
/// These inputs are private to projection, measured from real owned capacities.
pub(super) struct ProjectionDataBytes {
    pub(super) retained: u64,
    pub(super) future: u64,
}

/// Conservative requested layout for ArcInner<Backing>, including header/padding.
/// Rust1.93.1 Arc has two atomic-usize controls before data. Opaque allocator/OS
/// internals are separate qualification; this is not a physical process-peak bound.
const fn backing_bytes() -> u64 {
    (2 * size_of::<usize>() + align_of::<Backing>() - 1 + size_of::<Backing>()) as u64
}
fn quota(storage: u64, data: &ProjectionDataBytes) -> Result<u64, LifecycleRejection> {
    data.retained
        .checked_add(data.future)
        .and_then(|bytes| bytes.checked_add(backing_bytes()))
        .and_then(|covered| storage.checked_sub(covered))
        .ok_or(LifecycleRejection::Overloaded)
}
/// Complete only after worker construction scratch has dropped. Validate every
/// byte before taking the original claim or allocating the shared backing.
/// Failure leaves the exact caller claim in place; success issues one data guard
/// and one nonclone policy credit. No independent claim or pool is created.
pub(super) fn complete(
    storage: &mut Option<MemoryClaim>,
    data: ProjectionDataBytes,
) -> Result<(ProjectionDataFunding, ProjectionPolicyCredit), LifecycleRejection> {
    let available = quota(storage.as_ref().ok_or(LifecycleRejection::WrongPhase)?.bytes(), &data)?;
    let original = storage.take().ok_or(LifecycleRejection::WrongPhase)?;
    let backing = Arc::new(Backing { _storage: original });
    Ok((
        ProjectionDataFunding { backing: Some(Arc::clone(&backing)) },
        ProjectionPolicyCredit { backing: Some(backing), bytes: available },
    ))
}
fn release(backing: &mut Option<Arc<Backing>>) {
    if let Some(shared) = backing.take()
        && let Some(payload) = Arc::into_inner(shared)
    {
        // into_inner deallocates the last Arc control before this original claim.
        drop(payload);
    }
}
impl Drop for ProjectionDataFunding {
    fn drop(&mut self) {
        release(&mut self.backing);
    }
}
impl Drop for ProjectionPolicyCredit {
    fn drop(&mut self) {
        release(&mut self.backing);
    }
}
/// Supply backend owner tests with a genuinely funded, empty-data credit. The
/// supplied claim must come from the actual execution admission fixture; this
/// calls the same production quota constructor and grants no native authority.
#[cfg(test)]
pub(in crate::production) fn test_policy_credit(
    storage: MemoryClaim,
) -> Result<ProjectionPolicyCredit, LifecycleRejection> {
    let (data, credit) =
        complete(&mut Some(storage), ProjectionDataBytes { retained: 0, future: 0 })?;
    drop(data);
    Ok(credit)
}
#[cfg(test)]
#[path = "funding/tests.rs"]
pub(super) mod tests;
