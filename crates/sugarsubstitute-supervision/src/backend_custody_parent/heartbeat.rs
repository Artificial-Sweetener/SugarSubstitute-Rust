// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Demonstrate coordinator progress without competing with retained custody transactions.
use super::*;
use std::time::Duration;
use sugarsubstitute_execution::ExecutionClient;

/// A heartbeat owns its exact nonce until the corresponding receipt is consumed.
#[derive(Default)]
pub(super) struct Heartbeat {
    next: Option<Moment>,
    nonce: Option<u64>,
}
impl Heartbeat {
    /// Consume only the original request's authenticated response.
    pub(super) fn acknowledge(&mut self, nonce: u64) -> Result<(), NativeObservationError> {
        if self.nonce != Some(nonce) {
            return Err(reject("custody_heartbeat_correlation"));
        }
        self.nonce = None;
        Ok(())
    }
}
impl BackendCustodyParent {
    /// Progress the independent coordinator watchdog using this session's original transaction owner.
    /// Busy profile/backend requests keep their own nonce and are themselves progress evidence.
    /// # Errors
    /// Preserves native admission, clock and transport causes.
    pub fn maintain_liveness(
        &self,
        execution: &ExecutionClient,
    ) -> Result<(), BackendCustodyParentFailure> {
        if !self.matches_execution(execution) {
            return Err(self.failure(reject("custody_heartbeat_execution")));
        }
        self.consume(NonZeroUsize::new(32).unwrap_or(NonZeroUsize::MIN))?;
        let now = execution.now().map_err(|cause| {
            self.failure(NativeObservationError::new("custody_heartbeat_clock", cause))
        })?;
        self.try_access(|state| {
            if !state.established
                || state.closing
                || state.finish.is_some()
                || state.heartbeat.next.is_some_and(|next| now < next)
            {
                return Ok(());
            }
            state.heartbeat.next =
                Some(now.checked_add(Duration::from_secs(2)).map_err(|cause| {
                    NativeObservationError::new("custody_heartbeat_deadline", cause)
                })?);
            if state.pending.is_some()
                || state.receipt.is_some()
                || state.heartbeat.nonce.is_some()
                || state.read.is_some()
                || state.write.is_some()
            {
                return Ok(());
            }
            let until = now.checked_add(Duration::from_secs(20)).map_err(|cause| {
                NativeObservationError::new("custody_heartbeat_deadline", cause)
            })?;
            let nonce = state.nonce.checked_add(1).ok_or_else(|| reject("custody_parent_nonce"))?;
            state.nonce = nonce;
            state.heartbeat.nonce = Some(nonce);
            state.pending = Some((
                BackendCustodyFrame {
                    key: self.key(),
                    nonce,
                    command: BackendCustodyCommand::Heartbeat,
                },
                until,
            ));
            state.advance()
        })?;
        Ok(())
    }
    /// Borrow the next coordinator progress observation without granting any process authority.
    pub fn heartbeat_deadline(&self) -> Option<Moment> {
        self.state.as_ref().and_then(|state| {
            lock(state).as_ref().and_then(|state| {
                (!state.closing && state.finish.is_none()).then_some(state.heartbeat.next).flatten()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_current_heartbeat_can_release_its_transaction() {
        let mut owner = Heartbeat { next: Some(Moment::from_nanos(50)), nonce: Some(7) };
        assert!(owner.acknowledge(8).is_err());
        assert_eq!(owner.nonce, Some(7));
        assert!(owner.acknowledge(7).is_ok());
        assert!(owner.acknowledge(7).is_err());
        assert_eq!(owner.next, Some(Moment::from_nanos(50)));
    }
}
