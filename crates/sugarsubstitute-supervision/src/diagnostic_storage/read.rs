// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin actual bounded reads outside cancellable results and retain failed closure.
use super::*;

pub(super) struct RetainedRead {
    pub(super) id: u64,
    pub(super) native: Option<NativeId>,
    pub(super) object: Option<DiagnosticObject>,
    pub(super) facts: Option<DiagnosticReadFacts>,
    pub(super) candidate: Option<(DiagnosticEntryClass, [u8; 32])>,
}

/// Borrow one retained exact file under its original operation and memory charge.
/// Dropping this pin does not close the native owner or make its slot reusable.
pub struct DiagnosticReadLease {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
    pub(super) facts: DiagnosticReadFacts,
}
impl DiagnosticStorage {
    /// Open exact typed bytes and validate actual size/identity/digest before return.
    /// # Errors
    /// Rejects foreign namespace, excessive bound, saturated actual lease custody,
    /// expired original context or native failure; partial outputs remain retained.
    pub fn read(
        &mut self,
        object: DiagnosticObject,
        bound: u64,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticReadLease, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        let namespace = state.data_mut()?.namespace;
        if object.namespace().is_some_and(|value| Some(value) != namespace)
            || bound > object.bound()
            || bound == 0
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        payload_discard::intent::admit_read(
            &mut state,
            crate::durable_storage::protected::record_file::diagnostic_object_binding(
                object,
                operation.key.owner().namespace,
            ),
            operation.key,
        )?;
        let id = reserve(&mut state, operation.key, Some(object), None)?;
        let result = state.call_native(|port| port.open(object, bound, context))?;
        finish(&mut state, &self.shared, operation.key, id, bound, result, context)
    }
    /// Read a constructor-private observed candidate before semantic codec validation.
    /// # Errors
    /// Rejects foreign inventory/native custody, changed actual identity, oversized
    /// source or expired original operation. This grants no candidate deletion right.
    pub fn read_inventory(
        &mut self,
        entry: &DiagnosticInventoryEntry,
        bound: u64,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticReadLease, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if !Shared::ptr_eq(&self.shared, &entry.shared)
            || entry.namespace != operation.key.owner().namespace
            || !entry.observed.readable()
            || bound == 0
            || bound > entry.observed.class.bound()
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        payload_discard::intent::admit_read(
            &mut state,
            (entry.observed.class, entry.observed.digest),
            operation.key,
        )?;
        let id = reserve(
            &mut state,
            operation.key,
            None,
            Some((entry.observed.class, entry.observed.digest)),
        )?;
        let result =
            state.call_native(|port| port.open_inventory(&entry.observed, bound, context))?;
        finish(&mut state, &self.shared, operation.key, id, bound, result, context)
    }
}
fn reserve(
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
    object: Option<DiagnosticObject>,
    candidate: Option<(DiagnosticEntryClass, [u8; 32])>,
) -> Result<u64, DiagnosticStorageError> {
    if state.data_mut()?.namespace.is_none() || state.data_mut()?.capture.is_some() {
        return Err(error(key, Code::Closed));
    }
    if state.data_mut()?.reads.len() + state.data_mut()?.capture_reads >= DIAGNOSTIC_READ_BOUND {
        return Err(error(key, Code::Capacity));
    }
    let id = next_lease(state, key)?;
    // The retained slot exists before an external boundary can produce a handle.
    state.data_mut()?.reads.push(RetainedRead { id, native: None, object, facts: None, candidate });
    Ok(id)
}
fn finish(
    state: &mut StateAccess<'_>,
    shared: &Shared,
    key: LogicalStoreOperationKey,
    id: u64,
    bound: u64,
    result: Result<(NativeId, DiagnosticReadFacts), ClaimError>,
    context: &JobContext,
) -> Result<DiagnosticReadLease, DiagnosticStorageError> {
    let (native_id, facts) = result.map_err(|cause| native(key, Code::Read, cause))?;
    let retained = state
        .data_mut()?
        .reads
        .iter_mut()
        .find(|value| value.id == id)
        .ok_or_else(|| error(key, Code::InvalidIdentity))?;
    retained.native = Some(native_id);
    retained.facts = Some(facts);
    if native_id == 0 || facts.identity == [0; 24] || facts.bytes > bound {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    Ok(DiagnosticReadLease { shared: shared.clone(), operation: key, id, facts })
}
impl DiagnosticReadLease {
    /// Retain one charged complete source under the actual original file fence.
    /// # Errors
    /// Rejects changed identity, invalid context, saturation or incomplete read.
    pub fn whole_source(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticRecoverySource, DiagnosticStorageError> {
        let deadline = {
            let mut state = StateAccess::acquire(&self.shared, self.operation);
            check(&mut state, self.operation, context, false)?;
            state
                .data_mut()?
                .operation
                .as_ref()
                .ok_or_else(|| error(self.operation, Code::Closed))?
                .deadline
        };
        let shared = self.shared.clone();
        let operation = DiagnosticStorageOperation {
            shared: self.shared.clone(),
            key: self.operation,
            deadline,
        };
        source::whole(&shared, self, &operation, context)
    }
    /// Observe exact actual retained identity, content digest and byte length.
    pub const fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
    /// Bind a strictly decoded full object to the actual canonical inventory candidate.
    ///
    /// This cached boundary transition performs no reopen or filesystem mutation and
    /// grants no deletion authority. The original read custody and deadline remain.
    /// # Errors
    /// Rejects retired/foreign context, changed native facts, namespace/class/name
    /// mismatch or changed repeated semantic binding. Exact live repeats are idempotent.
    pub fn bind_object(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        if capture_intent::pins(state.data_mut()?, self.id) {
            return Err(error(self.operation, Code::Busy));
        }
        let namespace =
            state.data_mut()?.namespace.ok_or_else(|| error(self.operation, Code::Closed))?;
        if object.namespace().is_some_and(|actual| actual != namespace) {
            return Err(error(self.operation, Code::InvalidIdentity));
        }
        let index = state
            .data_mut()?
            .reads
            .iter()
            .position(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        let retained = &state.data_mut()?.reads[index];
        if retained.native.is_none_or(|id| id == 0)
            || retained.facts != Some(self.facts)
            || retained.object.is_some_and(|actual| actual != object)
        {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        if let Some(candidate) = retained.candidate {
            if candidate
                != crate::durable_storage::protected::record_file::diagnostic_object_binding(
                    object, namespace,
                )
            {
                return Err(error(self.operation, Code::InvalidIdentity));
            }
        } else if retained.object != Some(object) {
            return Err(error(self.operation, Code::InvalidIdentity));
        }
        check(&mut state, self.operation, context, false)?;
        state.data_mut()?.reads[index].object = Some(object);
        Ok(())
    }
    /// Read at most64KiB from the exact retained source at a checked byte offset.
    /// # Errors
    /// Rejects retired/foreign pins, oversized buffer or out-of-range offset, original
    /// deadline/cancellation and native short/failed reads. No allocation uses source size.
    pub fn read_chunk(
        &mut self,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        if output.len() > DIAGNOSTIC_IO_BYTES || offset > self.facts.bytes {
            return Err(error(self.operation, Code::Capacity));
        }
        let retained = state
            .data_mut()?
            .reads
            .iter()
            .find(|value| value.id == self.id)
            .ok_or_else(|| error(self.operation, Code::Closed))?;
        if retained.facts != Some(self.facts) {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        let id = retained.native.ok_or_else(|| error(self.operation, Code::Closed))?;
        let available = usize::try_from((self.facts.bytes - offset).min(output.len() as u64))
            .map_err(|_| error(self.operation, Code::Capacity))?;
        let actual = state
            .call_native(|port| port.read(id, offset, &mut output[..available], context))?
            .map_err(|cause| native(self.operation, Code::Read, cause))?;
        if actual != available {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        check(&mut state, self.operation, context, false)?;
        Ok(actual)
    }
    /// Checked-close the actual source before freeing its retained lease slot.
    /// # Errors
    /// Failed close/context validation retains both this pin and all native custody.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let Some(index) = state.data_mut()?.reads.iter().position(|value| value.id == self.id)
        else {
            return Ok(());
        };
        if summary_migration::intent::pins_read(&mut state, self.id)?
            || payload_discard::intent::pins_read(&mut state, self.id)?
            || capture_intent::pins(state.data_mut()?, self.id)
        {
            return Err(error(self.operation, Code::Busy));
        }
        if let Some(id) = state.data_mut()?.reads[index].native {
            state
                .call_native(|port| port.close(id, context))?
                .map_err(|cause| native(self.operation, Code::Retirement, cause))?;
        }
        state.data_mut()?.reads.remove(index);
        Ok(())
    }
}
