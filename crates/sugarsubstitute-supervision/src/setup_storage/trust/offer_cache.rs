// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist signed discovery bytes for reauthentication after process retirement.
mod codec;
use super::*;
use offered::OfferInputs;

/// Admission for bounded original, decoded and replacement cache buffers.
pub const OFFER_CACHE_RESERVATION_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_PAYLOAD_BYTES: usize = 16
    + 3 * sugarsubstitute_update::MAX_METADATA_BYTES
    + sugarsubstitute_contracts::update::MAX_OFFER_BYTES;

impl TrustStore {
    /// Save exact signed bytes under this installation/repository's cache namespace.
    /// This stores no local subscription, engine capabilities, healthy release facts or authority.
    /// Readers must supply fresh local facts and reauthenticate against current protected trust.
    /// Reserve `OFFER_CACHE_RESERVATION_BYTES` plus ordinary trust-operation capacity.
    /// # Errors
    /// Preserves unknown/corrupt files, earlier complete bytes and abandoned staging on failure.
    pub fn remember_offer(
        &self,
        offer: &AuthenticatedOffer,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        check(context)?;
        let _memory =
            context.memory().claim(OFFER_CACHE_RESERVATION_BYTES).map_err(io::Error::other)?;
        let authorization = offer.authorization(self);
        let payload = codec::encode(authorization.metadata, authorization.descriptor)?;
        let bytes = envelope::encode(
            envelope::Kind::Offer,
            &self.installation,
            &self.repository,
            &payload,
        )?;
        let leaf = name(authorization.request);
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            if let Some(previous) = self.read_offer(&leaf, protection)? {
                let payload = envelope::decode(
                    envelope::Kind::Offer,
                    &self.installation,
                    &self.repository,
                    &previous,
                )?;
                codec::validate(payload)?;
                if previous == bytes {
                    return check(context).map_err(TrustFailure::from);
                }
            }
            let staged = staging::unique_path(&self.root, "offer-pending")?;
            let mut file = protection.file(&staged, context)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            check(context)?;
            staging::replace_record(&staged, &self.root.path.join(leaf))?;
            check(context)?;
            Ok(())
        })
    }

    /// Reauthenticate cached bytes using current local facts and Unix UTC seconds.
    /// Missing cache returns `None`; invalid, expired or revoked evidence returns its cause.
    /// Cache bytes never restore or initialize signing trust and never provide request facts.
    /// Reserve cache and offer capacity plus ordinary trust-operation capacity.
    /// # Errors
    /// Preserves corrupt/foreign files and reports signature, rollback, expiry and storage errors.
    pub fn cached_offer(
        &self,
        request: Request,
        now: i64,
        context: &JobContext,
    ) -> Result<Option<AuthenticatedOffer>, TrustFailure> {
        check(context)?;
        let memory = context.memory().claim(OFFER_RESERVATION_BYTES).map_err(io::Error::other)?;
        let inputs = {
            let _cache =
                context.memory().claim(OFFER_CACHE_RESERVATION_BYTES).map_err(io::Error::other)?;
            self.protected(context, |protection| {
                let _writer = self.writer(protection)?;
                let Some(bytes) = self.read_offer(&name(&request), protection)? else {
                    return Ok(None);
                };
                let payload = envelope::decode(
                    envelope::Kind::Offer,
                    &self.installation,
                    &self.repository,
                    &bytes,
                )?;
                codec::decode(payload, request).map(Some).map_err(TrustFailure::from)
            })?
        };
        inputs
            .map(|inputs| AuthenticatedOffer::from_parts(self, inputs, memory, now, context))
            .transpose()
    }

    /// Pin a canonical cache input while checking bounds and protection; never inspect siblings.
    fn read_offer(&self, leaf: &str, protection: &Protection) -> io::Result<Option<Vec<u8>>> {
        let mut file = match self.root.read(leaf) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause),
        };
        protection.verify_file(&file)?;
        files::read_bounded(&mut file, envelope::Kind::Offer.limit()).map(Some)
    }
}

/// Route-derived flat names isolate subscriptions without persisting caller-owned request facts.
fn name(request: &Request) -> String {
    format!("offer-v1-{}", request.target_name())
}

#[cfg(test)]
mod tests;
