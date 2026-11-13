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
                authority: CandidateAuthority::Offer(Box::new(offer)),
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

impl SignedCandidate {
    /// Reopen the prepared candidate on its protected receipt when cached signed metadata is
    /// expired or missing (D1, D22). `request` holds this engine's facts for the current
    /// subscription and `now` reads the injected clock. All application bytes and saved choices
    /// are verified, and every receipt condition is checked, before this returns a candidate;
    /// trial admission checks the receipt again under run custody.
    /// Returns `None` when no candidate is prepared, it has no receipt, or its release is
    /// withheld; the accepted version then starts.
    /// # Errors
    /// Returns `ReceiptRefused` naming the failing condition, and preserves corruption,
    /// ownership, cancellation and storage failures. Nothing is published on any error.
    pub(crate) fn reopen_received(
        installation: &Path,
        trust: TrustStore,
        request: Request,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Option<Self>, TrustFailure> {
        let Some(receipt) = trust.update_receipt(context)? else { return Ok(None) };
        let session = InstallationSession::maintain(installation, context)?;
        let Some((candidate, configuration)) = PreparedCandidate::reopen_received(
            &session,
            &trust,
            &receipt,
            &request,
            now(),
            context,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            candidate,
            trust,
            authority: CandidateAuthority::Receipt { receipt: Box::new(receipt), request },
            _custody: CandidateCustody::Reopened {
                _session: Box::new(session),
                _configuration: Box::new(configuration),
            },
        }))
    }
}
