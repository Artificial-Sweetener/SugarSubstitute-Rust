// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Store signed discovery bytes under shared disk policy and reauthenticate every offline read.
mod codec;
use super::*;
use crate::setup_storage::cache::{self, Namespace};
use offered::OfferInputs;

/// Active-job capacity for bounded streaming publication through shared storage.
pub const OFFER_PUBLICATION_RESERVATION_BYTES: u64 = 16 * 1024 * 1024;
/// Capacity for retained offer inputs, bounded reading, shared-cache traversal and native custody.
pub const OFFER_LOOKUP_RESERVATION_BYTES: u64 = 24 * 1024 * 1024;
pub(super) const MAX_PAYLOAD_BYTES: usize = 16
    + 3 * sugarsubstitute_update::MAX_METADATA_BYTES
    + sugarsubstitute_contracts::update::MAX_OFFER_BYTES;
const RECORD: &str = "signed-offer";

impl TrustStore {
    /// Publish exact signed bytes in an independent cache generation after candidate preparation.
    /// Readers supply fresh local facts and reauthenticate against current protected trust.
    /// Signed inputs stream directly into admitted storage without another encoded allocation.
    /// Reserve `OFFER_PUBLICATION_RESERVATION_BYTES` alongside ordinary trust-operation capacity.
    /// # Errors
    /// Preserves pinned generations, damaged evidence, cancellation and storage failures.
    pub fn remember_offer(
        &self,
        offer: &AuthenticatedOffer,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        check(context)?;
        if !offer.belongs_to(self) {
            return Err(
                invalid("Cached offer belongs to another installation or repository").into()
            );
        }
        let authorization = offer.authorization(self);
        let length = codec::record_len(&authorization.metadata, authorization.descriptor)?;
        let mut lease = self.runtime_cache(context)?.fresh(
            Namespace::UpdateOffers,
            &self.offer_key(offer.authorization(self).request),
            envelope::Kind::Offer.limit() as u64,
            cache::utc_seconds()?,
            context,
        )?;
        lease.write_with(RECORD, length as u64, context, |output| {
            codec::write(
                &self.installation,
                &self.repository,
                authorization.metadata,
                authorization.descriptor,
                output,
            )
        })?;
        lease.publish(context)?;
        Ok(())
    }

    /// Reauthenticate cached bytes using current local facts and Unix UTC seconds.
    /// Missing storage returns `None`; invalid, expired or revoked evidence reports its cause.
    /// Cache bytes never restore signing trust or supply request facts. Reserve
    /// `OFFER_LOOKUP_RESERVATION_BYTES` plus ordinary trust-operation capacity.
    /// # Errors
    /// Preserves corrupt/foreign bytes and signature, rollback, expiry and storage failures.
    pub fn cached_offer(
        &self,
        request: Request,
        now: i64,
        context: &JobContext,
    ) -> Result<Option<AuthenticatedOffer>, TrustFailure> {
        check(context)?;
        let memory = context.memory().claim(OFFER_RESERVATION_BYTES).map_err(io::Error::other)?;
        let inputs = {
            // A bounded read may grow its allocation geometrically; decoded inputs are already
            // covered by the retained offer claim, and the read allocation retires before parsing.
            let _buffer = context.memory().claim(8 * 1024 * 1024).map_err(io::Error::other)?;
            let Some(lease) = self.runtime_cache(context)?.find(
                Namespace::UpdateOffers,
                &self.offer_key(&request),
                cache::utc_seconds()?,
                context,
            )?
            else {
                return Ok(None);
            };
            let bytes = lease.read(RECORD, context)?.read_bounded(envelope::Kind::Offer.limit())?;
            let payload = envelope::decode(
                envelope::Kind::Offer,
                &self.installation,
                &self.repository,
                &bytes,
            )?;
            codec::decode(payload, request)?
        };
        AuthenticatedOffer::from_parts(self, inputs, memory, now, context).map(Some)
    }

    /// Route and physical installation identities partition disposable generations independently
    /// from the protected trust directory. No current subscription facts are persisted here.
    fn offer_key(&self, request: &Request) -> String {
        cache::storage_key(
            format!(
                "offer-v1:{}:{:x}:{}",
                self.installation.as_str(),
                sha2::Sha256::digest(self.repository),
                request.target_name(),
            )
            .as_bytes(),
        )
    }
}
use sha2::Digest;

#[cfg(test)]
mod tests;
