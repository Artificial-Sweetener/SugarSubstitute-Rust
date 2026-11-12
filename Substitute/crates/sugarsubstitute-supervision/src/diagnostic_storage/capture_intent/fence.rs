// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow genuine pinned files and controls without exposing a native settlement constructor.
use super::*;

/// Synchronous borrow of the original writer and private canonical read slots.
/// Only the storage owner constructs this fence; it grants no allocation or native token.
pub struct DiagnosticCaptureFence<'a> {
    pub(super) shared: &'a Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
    pub(super) context: &'a JobContext,
}
impl DiagnosticCaptureFence<'_> {
    /// Borrow the actual original effect context; it cannot be renewed by the adapter.
    pub fn context(&self) -> &JobContext {
        self.context
    }
    /// Observe the full current operation as inert correlation data.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Compare the actual installed generation's same store and full operation.
    pub fn matches_generation(&self, generation: &ProtectedCaptureReadLease) -> bool {
        generation.matches_storage(self.shared, self.operation)
    }
    /// Borrow checked canonical Journal facts without projecting handle authority.
    pub fn journal(
        &self,
    ) -> Result<(DiagnosticObject, DiagnosticReadFacts), DiagnosticStorageError> {
        let state = lock(self.shared);
        let p = &retained(&state, self.id, self.operation)?.journal;
        Ok((p.object, p.facts))
    }
    /// Borrow checked canonical Summary facts without projecting handle authority.
    pub fn summary(
        &self,
    ) -> Result<(DiagnosticObject, DiagnosticReadFacts), DiagnosticStorageError> {
        let state = lock(self.shared);
        let p = &retained(&state, self.id, self.operation)?.summary;
        Ok((p.object, p.facts))
    }
    /// Observe the optional actual immutable Minidump pin; absence is not discard proof.
    pub fn payload(
        &self,
    ) -> Result<Option<(DiagnosticObject, DiagnosticReadFacts)>, DiagnosticStorageError> {
        let state = lock(self.shared);
        Ok(retained(&state, self.id, self.operation)?.payload.as_ref().map(|p| (p.object, p.facts)))
    }
    /// Check original controls and the exact retained read/delete pins for one native effect.
    /// This performs no file read or hashing. The same immutable native pins exclude
    /// external writes/deletion, and the capture intent excludes storage mutations.
    /// Full file identity/hash observations remain required around the source callback.
    /// # Errors
    /// Rejects original cancellation/expiry, a lost writer, closed Journal or changed slots.
    pub fn check_pinned_current(&mut self) -> Result<(), DiagnosticStorageError> {
        let mut access = StateAccess::acquire(self.shared, self.operation);
        access.check(self.operation, self.context, false)?;
        let data = access.data_mut()?;
        if !data.writer {
            return Err(error(self.operation, Code::Busy));
        }
        let intent = retained(data, self.id, self.operation)?;
        if intent.journal_closed {
            return Err(error(self.operation, Code::Closed));
        }
        for pin in [Some(&intent.journal), Some(&intent.summary), intent.payload.as_ref()]
            .into_iter()
            .flatten()
        {
            let read = data
                .reads
                .iter()
                .find(|read| read.id == pin.id)
                .ok_or_else(|| error(self.operation, Code::Closed))?;
            if read.native != Some(pin.native)
                || read.object != Some(pin.object)
                || read.facts != Some(pin.facts)
            {
                return Err(error(self.operation, Code::ChangedIdentity));
            }
        }
        Ok(())
    }
    /// Revalidate original controls and every exact pinned file outside all storage locks.
    /// # Errors
    /// Rejects original cancellation/expiry, lost writer, changed files or closed slots.
    pub fn check_current(&mut self) -> Result<(), DiagnosticStorageError> {
        let mut access = StateAccess::acquire(self.shared, self.operation);
        access.check(self.operation, self.context, false)?;
        if !access.data_mut()?.writer {
            return Err(error(self.operation, Code::Busy));
        }
        for index in 0..3 {
            let (native, facts, id) = {
                let intent = retained(access.data_mut()?, self.id, self.operation)?;
                let p = match index {
                    0 if !intent.journal_closed => Some(&intent.journal),
                    0 => None,
                    1 => Some(&intent.summary),
                    _ => intent.payload.as_ref(),
                };
                let Some(p) = p else {
                    continue;
                };
                (p.native, p.facts, p.id)
            };
            let read = access
                .data_mut()?
                .reads
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| error(self.operation, Code::Closed))?;
            if read.native != Some(native) || read.facts != Some(facts) {
                return Err(error(self.operation, Code::ChangedIdentity));
            }
            access.check(self.operation, self.context, false)?;
            let actual = access
                .call_native(|port| port.observe_file(native, self.context))?
                .map_err(|cause| native_error(self.operation, cause))?;
            if actual != facts {
                return Err(error(self.operation, Code::ChangedIdentity));
            }
            access.check(self.operation, self.context, false)?;
        }
        Ok(())
    }
}
fn native_error(key: LogicalStoreOperationKey, cause: ClaimError) -> DiagnosticStorageError {
    native(key, Code::Read, cause)
}
