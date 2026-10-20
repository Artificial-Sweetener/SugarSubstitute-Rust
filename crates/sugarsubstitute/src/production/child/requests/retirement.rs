// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence original finite correlation and retain checked never-issued withdrawal provenance.
use super::*;
/// Actual metadata withdrawal owns its original header/cutoff after native no-issue proof.
/// Construction stays with this ledger; copied request facts cannot produce the token.
pub(in crate::production::child) struct WithdrawnRequest {
    header: WireHeader,
    original: Moment,
    admitted: bool,
}
impl WithdrawnRequest {
    pub(in crate::production::child) const fn native_was_admitted(&self) -> bool {
        self.admitted
    }
    pub(in crate::production::child) const fn header(&self) -> WireHeader {
        self.header
    }
    pub(in crate::production::child) const fn original(&self) -> Moment {
        self.original
    }
}
impl ClientRequests {
    /// Mark only the exact original prepared output request before native admission.
    pub(in crate::production::child) fn output_pull(
        &mut self,
        header: WireHeader,
        running: bool,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Prepared || pending.purpose != Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.purpose = Purpose::OutputPull;
        pending.deferred_reply = running;
        Ok(())
    }
    /// Mark the finite frontend replacement request before any native queue admission.
    pub(in crate::production::child) fn frontend_reload(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Prepared || pending.purpose != Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.purpose = Purpose::FrontendReload;
        pending.deferred_reply = true;
        Ok(())
    }
    /// Designate the same finite output consumer's distinct user retry request.
    pub(in crate::production::child) fn backend_retry(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Prepared || pending.purpose != Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.purpose = Purpose::BackendRetry;
        pending.deferred_reply = true;
        Ok(())
    }
    /// Inspect only original queue admission; a prepared request has issued no native write.
    pub(in crate::production::child) fn prepared(
        &self,
        header: WireHeader,
    ) -> Result<bool, RequestError> {
        Ok(self.find(header)?.phase == Phase::Prepared)
    }
    /// Fence this exact existing admitted correlation without renewing its original cutoff.
    /// Returned bool distinguishes first retirement from its unchanged repeated request.
    pub(in crate::production::child) fn retire(
        &mut self,
        header: WireHeader,
        purpose: Purpose,
    ) -> Result<bool, RequestError> {
        let pending = self.find_mut(header)?;
        if pending.purpose != purpose || purpose == Purpose::Other {
            return Err(RequestError::Identity);
        }
        match pending.phase {
            Phase::Admitted => pending.phase = Phase::RetiringAdmitted,
            Phase::Received => pending.phase = Phase::RetiringReceived,
            Phase::RetiringAdmitted | Phase::RetiringReceived | Phase::RetiringUnissued => {
                return Ok(false);
            }
            _ => return Err(RequestError::Order),
        }
        Ok(true)
    }
    /// Adopt a retained original loan after its real receive cutoff rejection.
    /// A previously recorded receipt remains unchanged; no ordinary authority reopens.
    pub(in crate::production::child) fn retain_retiring_receipt(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        match self.find(header)?.phase {
            Phase::RetiringAdmitted => self.received_retiring(header),
            Phase::RetiringReceived => Ok(()),
            _ => Err(RequestError::Order),
        }
    }
    /// Borrow exact retirement routing; unknown headers retain strict rejection.
    pub(in crate::production::child) fn retiring(
        &self,
        header: WireHeader,
    ) -> Result<bool, RequestError> {
        Ok(matches!(
            self.find(header)?.phase,
            Phase::RetiringAdmitted | Phase::RetiringReceived | Phase::RetiringUnissued
        ))
    }
    /// Retain only an actual authenticated reply loan for disposal, never ordinary authority.
    pub(in crate::production::child) fn received_retiring(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::RetiringAdmitted || pending.purpose == Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.phase = Phase::RetiringReceived;
        Ok(())
    }
    /// Return only validated nonfinal acquisition correlation to await its final disposition.
    pub(in crate::production::child) fn disposed_progress(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::RetiringReceived || pending.purpose != Purpose::Acquisition {
            return Err(RequestError::Order);
        }
        pending.phase = Phase::RetiringAdmitted;
        Ok(())
    }
    /// Record a real no-issue veto only after this original buffer's callback quiesced.
    pub(in crate::production::child) fn revoked_retiring(
        &mut self,
        header: WireHeader,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::RetiringAdmitted || pending.purpose == Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.phase = Phase::RetiringUnissued;
        Ok(())
    }
    /// Borrow whether native issue was genuinely vetoed; this is not settlement authority.
    pub(in crate::production::child) fn never_issued(
        &self,
        header: WireHeader,
    ) -> Result<bool, RequestError> {
        Ok(self.find(header)?.phase == Phase::RetiringUnissued)
    }
    /// Check original correlation before its owner's genuine full settlement ACK.
    pub(in crate::production::child) fn can_settle(
        &self,
        header: WireHeader,
        purpose: Purpose,
    ) -> Result<(), RequestError> {
        let pending = self.find(header)?;
        if pending.purpose != purpose
            || !matches!(
                pending.phase,
                Phase::RetiringAdmitted | Phase::RetiringReceived | Phase::RetiringUnissued
            )
        {
            return Err(RequestError::Order);
        }
        Ok(())
    }
    /// Release only exact retired semantic correlation after actual owner proof.
    /// This releases no native buffer/job/lease and cannot stand in for that proof.
    pub(in crate::production::child) fn settled(
        &mut self,
        header: WireHeader,
        purpose: Purpose,
    ) -> Result<(), RequestError> {
        self.can_settle(header, purpose)?;
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|p| p.header == header))
            .ok_or(RequestError::Identity)?;
        *slot = None;
        Ok(())
    }

    /// Withdraw only a current Acquire that never entered native issue: either prepared,
    /// or the same original admission after its genuine returned pre-issue-vetoed buffer.
    pub(in crate::production::child) fn withdraw_acquisition(
        &mut self,
        header: WireHeader,
    ) -> Result<WithdrawnRequest, RequestError> {
        let pending = self.find(header)?;
        if pending.purpose != Purpose::Acquisition
            || !matches!(pending.phase, Phase::Prepared | Phase::RetiringUnissued)
        {
            return Err(RequestError::Order);
        }
        let proof = WithdrawnRequest {
            header,
            original: pending.original,
            admitted: pending.phase == Phase::RetiringUnissued,
        };
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|p| p.header == header))
            .ok_or(RequestError::Identity)?;
        *slot = None;
        Ok(proof)
    }
}
