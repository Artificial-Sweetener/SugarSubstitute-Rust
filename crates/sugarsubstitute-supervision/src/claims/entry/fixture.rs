// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow genuinely bound entry proof through real guard/publication policy for CPU tests.
use super::*;
use crate::{
    coordinator_entry::{
        election::{ElectionRecordOrigin, RetainedElectionProof},
        fixture::Harness,
    },
    protected_storage::account::ResolvedNamespace,
};
use std::error::Error;
use sugarsubstitute_contracts::execution::JobContext;
pub(crate) struct PublicationFixture {
    pub(crate) entry: Harness,
    claims: WindowsResourceClaims,
}
impl PublicationFixture {
    pub(crate) fn new(origin: ElectionRecordOrigin) -> Result<Self, Box<dyn Error>> {
        let mut entry = Harness::new()?;
        entry.discover()?;
        *crate::native_claims::lock(&entry.fake.origin) = origin;
        entry.elect()?;
        let mut claims = WindowsResourceClaims::new()?;
        entry.owner.prepare_claims(&mut claims)?;
        Ok(Self { entry, claims })
    }
    pub(crate) fn retained_proof(&self) -> Result<RetainedElectionProof, ClaimError> {
        self.claims
            .entry_initialization
            .as_ref()
            .map(RetainedElectionProof::retained)
            .ok_or_else(|| ClaimError::invalid("entry_fixture_bound_proof_missing"))
    }
    pub(crate) fn close_claim_admission(&mut self) {
        self.claims.close_admission();
    }
}
pub(crate) fn with_fresh_publication<T>(
    proof: &RetainedElectionProof,
    file: &HeldFile<128>,
    coordinator_inode: [u8; 24],
    context: &JobContext,
    resolved: &ResolvedNamespace,
    body: impl FnOnce(&FreshCoordinatorPublication<'_>, &HeldFile<128>) -> Result<T, ClaimError>,
) -> Result<T, ClaimError> {
    let guard = proof.lock_for_initialization(context, resolved)?;
    let permission = guard.publication(file, coordinator_inode)?;
    body(&permission, file)
}
