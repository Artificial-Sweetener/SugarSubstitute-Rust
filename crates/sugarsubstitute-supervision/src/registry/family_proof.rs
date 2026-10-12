// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify membership through the retained complete Job inventory owner.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionId, JobClass, Operation};

/// Retain one exact registry ticket, execution lifetime and native family.
/// Desktop profile and managed listener adapters enforce their own binding rules.
pub(crate) struct RetainedFamilyProof {
    pub(crate) identity: RegistryIdentity,
    pub(crate) ticket: LaunchTicket,
    execution: ExecutionId,
    shared: SharedOwner,
}
impl WindowsProcessRegistry {
    /// Obtain an opaque current family; public matching facts cannot recreate it.
    pub(crate) fn retained_family(
        &self,
        ticket: LaunchTicket,
    ) -> Result<RetainedFamilyProof, RegistryAdmissionError> {
        let record = self
            .records
            .get(self.index(ticket.process)?)
            .and_then(Option::as_ref)
            .ok_or(RegistryAdmissionError::Stale)?;
        if record.ticket != ticket {
            return Err(RegistryAdmissionError::Stale);
        }
        if !record.started
            || record.creation.is_some()
            || record.cleanup.is_some()
            || record.settled
            || record.shared.admission.load(Ordering::Acquire) == CLOSED
        {
            return Err(RegistryAdmissionError::Busy);
        }
        Ok(RetainedFamilyProof {
            identity: self.identity.clone(),
            ticket,
            execution: record.native_scope.owner_id(),
            shared: record.shared.clone(),
        })
    }
}
impl RetainedFamilyProof {
    /// Observe the exact family admission fence without blocking on native inventory.
    pub(crate) fn is_live(&self) -> bool {
        self.shared.admission.load(Ordering::Acquire) != CLOSED
    }
    /// Return actual verified PID/birth after complete current inventory refresh.
    /// The caller retains this proof throughout its bounded external operation and
    /// repeats qualification afterward. No raw process or Job authority is exposed.
    pub(crate) fn qualify_member(
        &self,
        pid: u32,
        context: &JobContext,
    ) -> Result<Identity, RegistryError> {
        self.check_context(context)?;
        if pid == 0 {
            return Err(RegistryError::invalid("family_member_pid"));
        }
        let mut native = lock(&self.shared.native);
        let family =
            native.family.as_mut().ok_or_else(|| RegistryError::invalid("family_retired"))?;
        if family.sealed || family.forced || family.closure_committed {
            return Err(RegistryError::invalid("family_retiring"));
        }
        let inventory = self.shared.port.inventory(family)?;
        if inventory.empty || family.incomplete || !listed_pid(&family.query, pid) {
            return Err(RegistryError::invalid("family_not_current_member"));
        }
        let member = family
            .members
            .iter()
            .flatten()
            .find(|member| member.identity.pid == pid)
            .ok_or_else(|| RegistryError::invalid("family_member_identity"))?;
        if member.exited || member.handle.is_none() || member.identity.birth == 0 {
            return Err(RegistryError::invalid("family_member_identity"));
        }
        let identity = member.identity;
        self.check_context(context)?;
        Ok(identity)
    }
    fn check_context(&self, context: &JobContext) -> Result<(), RegistryError> {
        if context.owner_id() != self.execution
            || context.spec().class != JobClass::InfrastructureLifecycle
            || !matches!(
                context.spec().operation,
                Operation::NativeLifecycle | Operation::Readiness
            )
            || context.cancellation_requested()
            || context.deadline().is_none_or(|limit| context.now() >= limit)
            || self.shared.admission.load(Ordering::Acquire) == CLOSED
        {
            Err(RegistryError::invalid("family_qualification_context"))
        } else {
            Ok(())
        }
    }
}

// The native inventory owner validates count, length, identities and IsProcessInJob.
// Older retained members cannot replace membership in the refreshed complete query.
fn listed_pid(query: &[usize], pid: u32) -> bool {
    let header_words = 8 / size_of::<usize>();
    let Some(header) = query.get(..header_words) else {
        return false;
    };
    let mut bytes = [0; 8];
    for (index, word) in header.iter().enumerate() {
        bytes[index * size_of::<usize>()..(index + 1) * size_of::<usize>()]
            .copy_from_slice(&word.to_ne_bytes());
    }
    let assigned = u32::from_ne_bytes(std::array::from_fn(|index| bytes[index])) as usize;
    let count = u32::from_ne_bytes(std::array::from_fn(|index| bytes[4 + index])) as usize;
    assigned == count
        && count <= native_process::MAX_MEMBERS
        && query
            .get(header_words..header_words.saturating_add(count))
            .is_some_and(|current| current.contains(&(pid as usize)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_pid_cannot_substitute_for_current_complete_inventory() {
        let mut query = [0usize; 258];
        let words = 8 / size_of::<usize>();
        let bytes = [1u32.to_ne_bytes(), 1u32.to_ne_bytes()].concat();
        for (index, chunk) in bytes.chunks_exact(size_of::<usize>()).enumerate() {
            query[index] = usize::from_ne_bytes(std::array::from_fn(|offset| chunk[offset]));
        }
        query[words] = 41;
        assert!(listed_pid(&query, 41));
        assert!(!listed_pid(&query, 42));
        assert!(!listed_pid(&query[..words], 41));
        query[0] = usize::MAX;
        assert!(!listed_pid(&query, 41));
    }
}
