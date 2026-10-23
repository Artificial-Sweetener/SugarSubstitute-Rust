// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain identical signed inputs for online and offline delivery and publication rechecks.
use super::*;
use sugarsubstitute_contracts::update::MAX_OFFER_BYTES;
use sugarsubstitute_update::MAX_METADATA_BYTES;

/// Admission for retained role/descriptor bytes and their bounded copy during construction.
pub const OFFER_RESERVATION_BYTES: u64 = 8 * 1024 * 1024;

/// Owned transport bytes; construction alone grants no authority.
pub(crate) struct OfferInputs {
    pub(crate) request: Request,
    pub(crate) timestamp: Vec<u8>,
    pub(crate) snapshot: Vec<u8>,
    pub(crate) targets: Vec<u8>,
    pub(crate) descriptor: Vec<u8>,
}
impl OfferInputs {
    /// Borrow unchanged role bytes for the repository verifier.
    fn metadata(&self) -> MetadataSet<'_> {
        MetadataSet { timestamp: &self.timestamp, snapshot: &self.snapshot, targets: &self.targets }
    }
}

/// Preparation evidence, not activation authority; request and signed bytes cannot be rebound.
pub struct AuthenticatedOffer {
    inputs: OfferInputs,
    offer: VerifiedOffer,
    _memory: MemoryClaim,
}
impl AuthenticatedOffer {
    /// Authenticate bounded embedded metadata and retain its exact bytes under memory admission.
    /// The caller supplies local subscription facts; metadata never supplies those facts.
    /// `now` is Unix UTC seconds. Reserve this owner's budget alongside trust operations.
    /// # Errors
    /// Returns no owner on signature, rollback, storage, input-bound or admission failure.
    pub fn authenticate(
        trust: &TrustStore,
        request: Request,
        metadata: MetadataSet<'_>,
        descriptor: &[u8],
        now: i64,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        check(context)?;
        validate_bounds(&metadata, descriptor)?;
        let memory = context.memory().claim(OFFER_RESERVATION_BYTES).map_err(io::Error::other)?;
        Self::from_parts(
            trust,
            OfferInputs {
                request,
                timestamp: metadata.timestamp.to_vec(),
                snapshot: metadata.snapshot.to_vec(),
                targets: metadata.targets.to_vec(),
                descriptor: descriptor.to_vec(),
            },
            memory,
            now,
            context,
        )
    }

    /// Transfer already admitted transport buffers without duplicating their retained memory.
    pub(crate) fn from_parts(
        trust: &TrustStore,
        inputs: OfferInputs,
        memory: MemoryClaim,
        now: i64,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        validate_bounds(&inputs.metadata(), &inputs.descriptor)?;
        let offer =
            trust.offer(inputs.metadata(), &inputs.request, &inputs.descriptor, now, context)?;
        Ok(Self { inputs, offer, _memory: memory })
    }

    /// Authenticated presentation facts and initial compatibility decision.
    pub fn offer(&self) -> &VerifiedOffer {
        &self.offer
    }

    /// Borrow the retained inputs for reauthorization against current installation trust.
    pub fn authorization<'a>(&'a self, trust: &'a TrustStore) -> UpdateAuthorization<'a> {
        UpdateAuthorization {
            trust,
            metadata: self.inputs.metadata(),
            request: &self.inputs.request,
            descriptor: &self.inputs.descriptor,
        }
    }
}

/// Bound every borrowed input before cloning or parsing untrusted bytes.
fn validate_bounds(metadata: &MetadataSet<'_>, descriptor: &[u8]) -> Result<(), TrustFailure> {
    if [metadata.timestamp, metadata.snapshot, metadata.targets]
        .iter()
        .any(|role| role.len() > MAX_METADATA_BYTES)
        || descriptor.len() > MAX_OFFER_BYTES
    {
        return Err(sugarsubstitute_update::Error::Bound.into());
    }
    Ok(())
}
