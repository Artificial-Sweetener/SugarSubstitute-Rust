// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve independent physical disposition, Journal deletion and checked native closure.
use super::*;
impl DiagnosticStorage {
    /// Settle the same native source outside locks, then delete only its exact Journal.
    /// Success requires actual delete, close and protected absence; it grants no Ack.
    /// Repeat only the retained intent under its unchanged original operation deadline.
    /// # Errors
    /// Source failure remains primary over a post-control failure; uncertain native/file
    /// effects preserve the intent, actual three pins and original funding for reconciliation.
    pub fn settle_capture<S: DiagnosticCaptureNativeSource>(
        &mut self,
        intent: &mut DiagnosticCaptureIntent,
        source: &mut S,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeletion, DiagnosticCaptureRetirementError<S::Error>> {
        if !Shared::ptr_eq(&self.shared, &intent.shared) || intent.operation != operation.key {
            return Err(DiagnosticCaptureRetirementError::admission(error(
                operation.key,
                Code::InvalidIdentity,
            )));
        }
        drop(self.check(operation, context).map_err(DiagnosticCaptureRetirementError::admission)?);
        let slot = super::types::CaptureErrorSlot::<S::Error>::new(&self.shared, operation.key)
            .map_err(DiagnosticCaptureRetirementError::admission)?;
        let mut fence = DiagnosticCaptureFence {
            shared: &self.shared,
            operation: operation.key,
            id: intent.id,
            context,
        };
        let (_, slot) = checked(fence.check_current(), slot)?;
        let pre = (|| {
            let mut state = lock(&self.shared);
            let p = retained_mut(&mut state, intent.id, operation.key)?;
            if p.callback_active {
                return Err(error(operation.key, Code::Busy));
            }
            if !p.native_settled {
                p.callback_active = true;
            }
            Ok(p.native_settled)
        })();
        let (settled, mut slot) = checked(pre, slot)?;
        if !settled {
            let mut callback = Callback {
                shared: &self.shared,
                id: intent.id,
                key: operation.key,
                returned: false,
            };
            let result = source.settle(&mut fence);
            callback.returned = true;
            let adopted: Result<(), DiagnosticStorageError> = (|| {
                let mut state = lock(&self.shared);
                let p = retained_mut(&mut state, intent.id, operation.key)?;
                p.callback_active = false;
                if result.is_ok() {
                    p.native_settled = true;
                }
                Ok(())
            })();
            // The owned source result exists before any lazy provider observation.
            slot = observe_source(
                &self.shared,
                result,
                || {
                    let post = fence.check_current();
                    match adopted {
                        Err(cause) => Err(cause.with_post_control(post.err())),
                        Ok(()) => post,
                    }
                },
                slot,
            )?;
        }
        checked(self.finish_capture_journal(intent, operation, context), slot)
            .map(|(result, _)| result)
    }
    fn finish_capture_journal(
        &mut self,
        intent: &mut DiagnosticCaptureIntent,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeletion, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        let p = retained(state.data_mut()?, intent.id, operation.key)?;
        if !p.native_settled || p.callback_active {
            return Err(error(operation.key, Code::Busy));
        }
        let (native_id, object, facts, id) =
            (p.journal.native, p.journal.object, p.journal.facts, p.journal.id);
        if !p.journal_closed {
            if !p.delete_issued {
                retained_mut(state.data_mut()?, intent.id, operation.key)?.delete_attempted = true;
                state
                    .call_native(|port| port.delete(native_id, object, facts, context))?
                    .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
                retained_mut(state.data_mut()?, intent.id, operation.key)?.delete_issued = true;
                state.check(operation.key, context, false)?;
            }
            state
                .call_native(|port| port.close(native_id, context))?
                .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
            retained_mut(state.data_mut()?, intent.id, operation.key)?.journal_closed = true;
            let read = state
                .data_mut()?
                .reads
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            read.native = None;
            state.check(operation.key, context, false)?;
        }
        if !state
            .call_native(|port| port.absent(object, context))?
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?
        {
            return Err(error(operation.key, Code::DeleteUncertain));
        }
        state.check(operation.key, context, false)?;
        state.data_mut()?.reads.retain(|r| r.id != id);
        state.data_mut()?.capture = None;
        state.data_mut()?.inventory_complete = false;
        Ok(DiagnosticDeletion { object, facts })
    }
}
pub(in crate::diagnostic_storage) fn observe_source<E>(
    original: &Shared,
    result: Result<(), E>,
    observe: impl FnOnce() -> Result<(), DiagnosticStorageError>,
    slot: super::types::CaptureErrorSlot<E>,
) -> Result<super::types::CaptureErrorSlot<E>, DiagnosticCaptureRetirementError<E>> {
    let post = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(observe)) {
        Ok(observed) => observed.map_err(super::types::PostFailure::Storage),
        Err(payload) => {
            let mut state = lock(original);
            state.closed = true;
            state.allocation.close();
            drop(state);
            Err(super::types::PostFailure::Panicked(Mutex::new(payload)))
        }
    };
    match result {
        Err(cause) => Err(slot.finish(Some(cause), post.err())),
        Ok(()) => match post {
            Err(cause) => Err(slot.finish(None, Some(cause))),
            Ok(()) => Ok(slot),
        },
    }
}
fn checked<T, E>(
    result: Result<T, DiagnosticStorageError>,
    slot: super::types::CaptureErrorSlot<E>,
) -> Result<(T, super::types::CaptureErrorSlot<E>), DiagnosticCaptureRetirementError<E>> {
    match result {
        Ok(value) => Ok((value, slot)),
        Err(cause) => Err(slot.finish(None, Some(super::types::PostFailure::Storage(cause)))),
    }
}
struct Callback<'a> {
    shared: &'a Shared,
    id: u64,
    key: LogicalStoreOperationKey,
    returned: bool,
}
impl Drop for Callback<'_> {
    fn drop(&mut self) {
        if !self.returned {
            let mut state = lock(self.shared);
            if let Ok(p) = retained_mut(&mut state, self.id, self.key) {
                p.callback_active = false;
            }
            state.closed = true;
            state.allocation.close();
        }
    }
}
