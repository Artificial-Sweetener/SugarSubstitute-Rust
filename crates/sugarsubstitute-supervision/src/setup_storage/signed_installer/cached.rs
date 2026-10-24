// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore a completed candidate from protected native publication, with archive repair fallback.
use super::*;

/// Missing files retain authenticated offer inputs for the ordinary delivery and repair path.
pub(crate) enum CachedCandidate {
    Ready(Box<SignedCandidate>),
    Missing(Box<TrustStore>, Box<AuthenticatedOffer>),
}
impl SignedCandidate {
    /// Reopen completed preparation without reading the archive or provisioning workspace.
    /// All application bytes, saved choices and current authority are checked before return.
    /// Only absence requests archive repair; corruption, revocation and ownership conflicts fail.
    pub(crate) fn reopen_cached(
        installation: &Path,
        trust: TrustStore,
        offer: AuthenticatedOffer,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<CachedCandidate, TrustFailure> {
        let session = InstallationSession::maintain(installation, context)?;
        let reopened = PreparedCandidate::reopen(&session, &trust, &offer, now, context);
        match reopened {
            Ok(Some((candidate, configuration))) => Ok(CachedCandidate::Ready(Box::new(Self {
                candidate,
                trust,
                offer,
                _custody: CandidateCustody::Reopened {
                    _session: Box::new(session),
                    _configuration: Box::new(configuration),
                },
            }))),
            Ok(None) => Ok(CachedCandidate::Missing(Box::new(trust), Box::new(offer))),
            Err(TrustFailure::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {
                Ok(CachedCandidate::Missing(Box::new(trust), Box::new(offer)))
            }
            Err(cause) => Err(cause),
        }
    }
}
