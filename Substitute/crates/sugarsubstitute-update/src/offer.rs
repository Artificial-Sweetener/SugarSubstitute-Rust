// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind subscription descriptors and immutable packages to the same authenticated metadata set.
use crate::{Error, RepositoryTrust, Target};
use sugarsubstitute_contracts::update::{Admission, MAX_OFFER_BYTES, Offer, Request};

/// Authenticated descriptor and its separately authenticated package identity.
pub struct VerifiedOffer {
    offer: Offer,
    admission: Admission,
    package: Target,
}
impl VerifiedOffer {
    /// Product compatibility decision; activation/data migration checks are still required.
    pub fn admission(&self) -> Admission {
        self.admission
    }
    /// Signed version and release identity.
    pub fn offer(&self) -> &Offer {
        &self.offer
    }
    /// Drop the package target's validity window after a clock-independent verification.
    pub(crate) fn pinned(self) -> Self {
        Self { package: self.package.pinned(), ..self }
    }
    /// Consume an admitted offer to begin verification of its exact immutable package.
    /// # Errors
    /// Rejects preparation when the package requires a newer engine/installer or is current.
    pub fn into_package(self) -> Result<Target, Error> {
        if self.admission != Admission::Prepare {
            return Err(Error::Admission);
        }
        Ok(self.package)
    }
}
impl RepositoryTrust {
    /// Verify descriptor bytes and bind their package digest/length to the same signed targets.
    /// # Errors
    /// Rejects oversized/altered descriptors, cross-channel routing, incompatible identities,
    /// missing package metadata, expiration and release rollback. No downloaded bytes execute.
    pub fn offer(&self, request: &Request, bytes: &[u8], now: i64) -> Result<VerifiedOffer, Error> {
        if bytes.len() > MAX_OFFER_BYTES {
            return Err(Error::Bound);
        }
        let mut descriptor = self.target(&request.target_name(), now)?.verifier(now)?;
        descriptor.push(bytes)?;
        descriptor.finish(now)?;
        let offer = Offer::decode(bytes).map_err(Error::Offer)?;
        let admission = offer.admission(request).map_err(Error::Offer)?;
        let package = self.target(&offer.package_name(), now)?;
        if package.length() != offer.package_bytes() || package.digest() != offer.package_digest() {
            return Err(Error::Digest);
        }
        Ok(VerifiedOffer { offer, admission, package })
    }
}
