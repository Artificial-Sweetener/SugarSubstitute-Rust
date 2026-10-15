// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual empty desktop Job before profile Held publication.
use super::*;
use crate::{
    BackendCustodyParent,
    native_process::{FamilyStorage, custody_transfer::CustodyDestination},
};
use sugarsubstitute_contracts::{
    backend_custody::ProfileCustodyKey, execution::Moment, lifecycle::RunKey,
};

/// Actual native family/destination precede their original finite memory custody.
pub(crate) struct PreparedDesktop {
    pub(crate) family: Option<FamilyStorage>,
    pub(crate) destination: Option<CustodyDestination>,
    pub(crate) parent: BackendCustodyParent,
    pub(crate) client: ExecutionClient,
    pub(crate) identity: RegistryIdentity,
    pub(crate) key: ProfileCustodyKey,
    pub(crate) until: Moment,
    pub(crate) prepared_nonce: Option<u64>,
    pub(crate) queued_prepare:
        Option<sugarsubstitute_contracts::backend_custody::BackendCustodyCommand>,
    pub(crate) prepared_ack: Option<crate::BackendCustodyAcknowledgement>,
    pub(crate) published_nonce: Option<u64>,
    pub(crate) published_ack: Option<crate::BackendCustodyAcknowledgement>,
    port: Arc<dyn NativePort>,
}
/// Allocate only for a genuinely prepared profile; keep its original metadata charge inseparable.
pub(crate) struct PreparedDesktopCarrier {
    pub(crate) value: Box<PreparedDesktop>,
    pub(crate) memory: Arc<sugarsubstitute_contracts::execution::MemoryClaim>,
}
impl PreparedDesktopCarrier {
    /// Count the additional profile-only allocation and all original bounded metadata first.
    pub(crate) const fn metadata_bytes() -> u64 {
        crate::profiles::carrier_metadata_bytes()
    }
    /// Reject before moving the original pin or allocating if its existing claim is insufficient.
    pub(crate) fn new(
        value: &mut Option<PreparedDesktop>,
        memory: Arc<sugarsubstitute_contracts::execution::MemoryClaim>,
    ) -> Result<Self, RegistryError> {
        if memory.bytes() < Self::metadata_bytes() {
            return Err(RegistryError::invalid("profile_carrier_metadata_capacity"));
        }
        let value = value
            .take()
            .ok_or_else(|| RegistryError::invalid("profile_carrier_original_missing"))?;
        Ok(Self { value: Box::new(value), memory })
    }
}
impl WindowsProcessRegistry {
    /// Close only this actual never-created placeholder and observe its original watch cancellation.
    pub(crate) fn retire_prepared_profile(
        &mut self,
        run: RunKey,
    ) -> Result<bool, RegistryAdmissionError> {
        let Some(index) = self.records.iter().position(|record| {
            record.as_ref().is_some_and(|record| {
                record.ticket.binding == FamilyBinding::Desktop(run)
                    && !record.started
                    && record.creation.is_none()
                    && lock(&record.shared.native).pending_input.is_none()
                    && lock(&record.shared.native).family.is_none()
            })
        }) else {
            return Ok(true);
        };
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        record.shared.admission.store(CLOSED, Ordering::Release);
        record.closed_native = true;
        record.rejected_launch = true;
        if let Some(permit) = lock(&record.shared.permit).take() {
            record.root_watch = Some(permit.cancel_unbound());
        }
        Ok(record.settled)
    }
    /// Reserve a unique registry family identity before actual profile/job preparation.
    /// The lexical identity alone grants neither process nor publication authority.
    pub(crate) fn reserve_profile_family(
        &mut self,
        run: RunKey,
        parent: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
        execution: &ExecutionClient,
        native_scope_spec: sugarsubstitute_contracts::execution::ScopeSpec,
        root_watch: JobSpec,
    ) -> Result<(RegistryIdentity, ProfileCustodyKey, Arc<dyn NativePort>), RegistryAdmissionError>
    {
        if !self.launch_open {
            return Err(RegistryAdmissionError::Closed);
        }
        if run.owner != self.owner || parent.owner() != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        if self.records.iter().flatten().any(|record| {
            matches!(record.ticket.binding,
            FamilyBinding::Desktop(old) if old.instance==run.instance)
        }) {
            return Err(RegistryAdmissionError::Busy);
        }
        let key = ProfileCustodyKey::new(run, self.next_process)
            .map_err(|_| RegistryAdmissionError::IdentityExhausted)?;
        self.next_process =
            self.next_process.checked_add(1).ok_or(RegistryAdmissionError::IdentityExhausted)?;
        submission::validate_watch(root_watch)?;
        if native_scope_spec.deadline.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let index = self
            .records
            .iter()
            .position(Option::is_none)
            .ok_or(RegistryAdmissionError::Saturated)?;
        let count = self
            .records
            .iter()
            .flatten()
            .filter(|record| matches!(record.ticket.binding, FamilyBinding::Desktop(_)))
            .count();
        if count >= 64 {
            return Err(RegistryAdmissionError::Saturated);
        }
        let native_scope = execution
            .create_native_scope(native_scope_spec, None)
            .map_err(RegistryAdmissionError::Execution)?;
        let permit = execution
            .reserve_process_watch(
                &native_scope,
                root_watch,
                sugarsubstitute_execution::windows::NativeCorrelation::new(key.registry_process())
                    .map_err(|_| RegistryAdmissionError::InvalidSpecification)?,
            )
            .map_err(RegistryAdmissionError::Execution)?;
        let shared = SharedOwner::new(
            Shared {
                admission: AtomicU8::new(OPEN),
                native: Mutex::new(NativeState {
                    establishment: None,
                    custody_destination: None,
                    custody_release: None,
                    pending_input: None,
                    family: None,
                    watch: None,
                    creation: None,
                    cleanup: None,
                    failure: None,
                    forced: false,
                    port: self.port.clone(),
                }),
                port: self.port.clone(),
                permit: Mutex::new(Some(permit)),
            },
            self.identity.clone(),
        );
        let ticket = LaunchTicket {
            process: sugarsubstitute_contracts::lifecycle::ProcessKey {
                owner: self.owner,
                id: sugarsubstitute_contracts::lifecycle::ProcessId::new(key.registry_process())
                    .map_err(|_| RegistryAdmissionError::IdentityExhausted)?,
            },
            binding: FamilyBinding::Desktop(run),
        };
        let root_spec = (native_scope.id(), root_watch.id);
        self.records[index] = Some(Record::pending(ticket, shared, native_scope, root_spec));
        Ok((self.identity.clone(), key, self.port.clone()))
    }
}
impl PreparedDesktop {
    #[cfg(test)]
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>()
    }
    /// Retire same local prepared Job/destination only after actual helper release.
    pub(crate) fn close_released(
        &mut self,
        ack: &crate::BackendCustodyAcknowledgement,
        context: &JobContext,
    ) -> Result<(), RegistryError> {
        if context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err(RegistryError::invalid("profile_prepared_cleanup_expired"));
        }
        if let Some(family) = &mut self.family {
            self.port.close(family)?;
        }
        self.family = None;
        if let Some(destination) = &mut self.destination {
            destination.close_profile_released(ack, self.key)?;
        }
        self.destination = None;
        Ok(())
    }
    /// Close an untransferred empty Job only inside the original checked cleanup operation.
    pub(crate) fn close_unissued(&mut self, context: &JobContext) -> Result<(), RegistryError> {
        if self.prepared_nonce.is_some() || self.queued_prepare.is_some() {
            return Err(RegistryError::invalid("profile_prepared_remote_outputs"));
        }
        if context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err(RegistryError::invalid("profile_prepared_cleanup_expired"));
        }
        if let Some(family) = &mut self.family {
            self.port.close(family)?;
        }
        self.family = None;
        Ok(())
    }
    /// Prepare the genuine empty native Job under the actual2Mi profile operation.
    pub(crate) fn new(
        identity: RegistryIdentity,
        key: ProfileCustodyKey,
        parent: BackendCustodyParent,
        port: Arc<dyn NativePort>,
        client: &ExecutionClient,
        context: &JobContext,
    ) -> Result<Self, RegistryError> {
        if !parent.matches_execution(client) {
            return Err(RegistryError::invalid("profile_prepared_execution"));
        }
        parent.check().map_err(|cause| RegistryError::new("profile_prepared_parent", cause))?;
        let until =
            context.deadline().ok_or_else(|| RegistryError::invalid("profile_prepared_cutoff"))?;
        let family = Family::allocate_owned(context.memory())?;
        let mut value = Self {
            family: Some(family),
            destination: Some(CustodyDestination::new(&parent)),
            parent,
            client: client.clone(),
            identity,
            key,
            until,
            prepared_nonce: None,
            queued_prepare: None,
            prepared_ack: None,
            published_nonce: None,
            published_ack: None,
            port,
        };
        value.port.prepare_job(
            value
                .family
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("profile_prepared_family"))?,
        )?;
        Ok(value)
    }
}
impl Drop for PreparedDesktop {
    fn drop(&mut self) {
        if let Some(family) = self.family.take() {
            tracing::error!(
                operation = "profile_prepared_abandonment",
                "Unresolved prepared profile native custody retained"
            );
            std::mem::forget(family);
        }
    }
}
