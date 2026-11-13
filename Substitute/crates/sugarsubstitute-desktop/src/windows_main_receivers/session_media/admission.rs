// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve bounded native media credit before adopting an actual resource deferral.
use super::*;
impl Media {
    /// Validate native request/source facts before creating or retaining a deferral.
    pub(crate) fn requested(&self, evidence: Evidence<'_>) -> Option<(Requested, MemoryClaim)> {
        if self.state.borrow().closed {
            return None;
        }
        let claim = match self.memory.claim(REQUEST_BYTES) {
            Ok(claim) => claim,
            Err(cause) => {
                tracing::warn!(operation="session_media_admission",identity=?self.identity,cause=%cause,
                "Retained media request exceeded its independent metadata admission");
                return None;
            }
        };
        self.endpoint.admit(evidence).map(|requested| (requested, claim))
    }
    /// Adopt actual native outputs before interpreting HRESULT; no native getter spans a registry borrow.
    pub(crate) fn defer(
        &self,
        requested: Requested,
        claim: MemoryClaim,
        args: &ICoreWebView2WebResourceRequestedEventArgs,
        environment: &ICoreWebView2Environment,
        now: Instant,
    ) -> Result<bool, DesktopError> {
        let deadline = now
            .checked_add(LIFETIME)
            .ok_or_else(|| DesktopError::message("session_media_deadline", "clock overflow"))?;
        let (index, id) = {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Ok(false);
            }
            if !requested.head && state.get_occupancy() >= GET_CAPACITY {
                tracing::warn!(operation="session_media_overload",identity=?self.identity,metadata_reserved=true,
                    "Retained media kept one original deferral credit for metadata");
                return Ok(false);
            }
            let Some(index) = state.slots.iter().position(|slot| matches!(slot, Slot::Vacant))
            else {
                tracing::warn!(operation="session_media_overload",identity=?self.identity,
                    "Retained media refused more than two active and eight queued requests");
                return Ok(false);
            };
            let number = state.next.checked_add(1).ok_or_else(|| {
                DesktopError::message("session_media_identity", "counter exhausted")
            })?;
            let id = NonZeroU64::new(number)
                .ok_or_else(|| DesktopError::message("session_media_identity", "zero counter"))?;
            state.next = number;
            state.slots[index] = Slot::Opening { id, head: requested.head };
            (index, id)
        };
        let mut raw = std::ptr::null_mut();
        // SAFETY: Live callback arguments write their deferral; the reserved slot precedes possible reentrancy.
        let status = unsafe { (args.vtable().GetDeferral)(args.as_raw(), &mut raw) };
        // SAFETY: A non-null output is the actual interface reference transferred by this native getter.
        let deferral = (!raw.is_null()).then(|| unsafe { ICoreWebView2Deferral::from_raw(raw) });
        let mut state = self.state.borrow_mut();
        if !matches!(state.slots[index],Slot::Opening {id:value,..} if value==id) {
            return Err(DesktopError::message(
                "session_media_deferral",
                "reserved native slot changed",
            ));
        }
        let Some(deferral) = deferral else {
            state.slots[index] = Slot::Vacant;
            return Err(DesktopError::new(
                "session_media_deferral",
                status.ok().err().unwrap_or_else(|| windows::core::Error::from(E_POINTER)),
            ));
        };
        let failure =
            status.ok().err().map(|cause| DesktopError::new("session_media_deferral", cause));
        let cancelled = state.closed || failure.is_some();
        let range = requested.range;
        let head = requested.head;
        let observation = SessionMediaObservation::new(
            self.identity,
            id,
            self.bootstrap.webview().into(),
            requested,
            deadline,
            claim,
        );
        state.slots[index] = Slot::Pending(Pending {
            id,
            request: Some(observation),
            args: args.clone(),
            deferral,
            environment: environment.clone(),
            deadline,
            range,
            head,
            active: false,
            cancelled,
            cancel_sent: false,
            response: None,
            responded: false,
        });
        if let Some(cause) = failure {
            DesktopError::retain_first(&mut state.failure, cause);
        }
        drop(state);
        self.events.wake();
        Ok(true)
    }
}
