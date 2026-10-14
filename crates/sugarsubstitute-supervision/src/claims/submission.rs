// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite work after fixed inventory and byte-bound checks.
use super::*;
impl WindowsResourceClaims {
    /// Resolve account/installation identity and acquire the fresh coordinator generation.
    /// Requires exactly CLAIM_POOL_BYTES. A stopped job may still report an acquired claim.
    pub fn submit_initialize(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        installation: PathBuf,
    ) -> Result<JobId, ClaimAdmissionError> {
        if self.initialization_started || !self.open.load(Ordering::Acquire) {
            return Err(ClaimAdmissionError::Closed);
        }
        if let Some(proof) = &self.entry_initialization {
            proof.validate_live().map_err(ClaimAdmissionError::Native)?;
            if proof.execution != scope.owner_id()
                || spec.deadline.is_none_or(|until| until > proof.budget.until())
            {
                return Err(ClaimAdmissionError::Identity);
            }
        }
        let entry_proof = self
            .entry_initialization
            .as_ref()
            .map(crate::coordinator_entry::election::RetainedElectionProof::retained);
        let index = self.admit_index(spec, true)?;
        if !installation.is_absolute() || installation.as_os_str().len() > 32766 * 3 {
            return Err(ClaimAdmissionError::Specification);
        }
        let installation = PathBuf::from(installation.as_os_str());
        let shared = self.root.clone();
        let port = self.port.clone();
        let open = self.open.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let pool = pool::Pool::new(context.memory()).map_err(|e| shared.fail(e))?;
                let mut root = lock(&shared.value);
                root.execution_owner = Some(context.owner_id());
                root.entry_election = entry_proof;
                root.pool = Some(pool);
                if context.cancellation_requested() || !open.load(Ordering::Acquire) {
                    return Ok(());
                }
                port.initialize(&mut root, &installation, &context).map_err(|e| shared.fail(e))
            })
            .map_err(ClaimAdmissionError::Execution)?;
        self.initialization_started = true;
        self.initialization_pending = true;
        self.execution_owner = Some(scope.owner_id());
        self.initial_scope = Some(scope.id());
        self.pending[index] =
            Some(Pending { job, action: Action::Initialize, target: ClaimTarget::Coordinator });
        Ok(spec.id)
    }
    /// Admit canonical filesystem resolution without launching any process.
    /// Retains at most64 resolutions and denies unknown/remote identities in the native adapter.
    pub fn submit_resolve_launch(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        configuration: LaunchConfiguration,
    ) -> Result<ResolutionTicket, ClaimAdmissionError> {
        self.usable()?;
        self.validate_scope(scope)?;
        let input_bytes = configuration.root().as_os_str().len()
            + configuration.interpreter().as_os_str().len()
            + std::mem::size_of_val(configuration.arguments())
            + configuration.arguments().iter().map(String::len).sum::<usize>();
        if input_bytes as u64 > pool::SLOT_BYTES / 4 {
            return Err(ClaimAdmissionError::Specification);
        }
        let configuration = LaunchConfiguration::new(
            configuration.root().to_path_buf(),
            configuration.interpreter().to_path_buf(),
            configuration.arguments().iter().map(|argument| argument.as_str().to_owned()).collect(),
            configuration.port().get(),
        )
        .map_err(|_| ClaimAdmissionError::Specification)?;
        let pending = self.admit_index(spec, false)?;
        let index = self
            .resolutions
            .iter()
            .position(Option::is_none)
            .ok_or(ClaimAdmissionError::Saturated)?;
        let next = self.next_resolution.checked_add(1).ok_or(ClaimAdmissionError::Identity)?;
        let (slot, sid) = {
            let root = lock(&self.root.value);
            (
                root.pool
                    .as_ref()
                    .and_then(pool::Pool::acquire)
                    .ok_or(ClaimAdmissionError::Saturated)?,
                root.sid.clone(),
            )
        };
        if configuration_bytes(&configuration) > pool::SLOT_BYTES / 4 {
            return Err(ClaimAdmissionError::Specification);
        }
        let shared = Shared::new(Some(Resolution::new(configuration, slot)));
        let worker = shared.clone();
        let port = self.port.clone();
        let open = self.open.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(CLAIM_OPERATION_BYTES)
                    .map_err(|e| worker.fail(ClaimError::new("resolve_scratch", e)))?;
                if context.cancellation_requested() || !open.load(Ordering::Acquire) {
                    return Ok(());
                }
                let mut value = lock(&worker.value);
                let resolution = value
                    .as_mut()
                    .ok_or_else(|| worker.fail(ClaimError::invalid("missing_resolution")))?;
                port.resolve(resolution, &sid).map_err(|e| worker.fail(e))
            })
            .map_err(ClaimAdmissionError::Execution)?;
        let ticket = ResolutionTicket { id: self.next_resolution, origin: self.origin.clone() };
        self.next_resolution = next;
        self.resolutions[index] = Some(ResolutionEntry {
            ticket: ticket.clone(),
            shared,
            proof: None,
            pending: true,
            failed: false,
        });
        self.pending[pending] = Some(Pending {
            job,
            action: Action::Resolve(index),
            target: ClaimTarget::Resolution(ticket.clone()),
        });
        Ok(ticket)
    }
    /// Move one successful resolution into an exact backend generation and acquire exclusion.
    /// A late cancelled acquisition stays retained and must use checked release.
    pub fn submit_backend_claim(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        resource: ResourceKey,
        ticket: &ResolutionTicket,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.usable()?;
        self.validate_scope(scope)?;
        if Some(resource.owner) != self.owner {
            return Err(ClaimAdmissionError::Identity);
        }
        if self.backends.iter().flatten().any(|entry| entry.resource.id == resource.id) {
            return Err(ClaimAdmissionError::Busy);
        }
        let pending = self.admit_index(spec, false)?;
        let index =
            self.backends.iter().position(Option::is_none).ok_or(ClaimAdmissionError::Saturated)?;
        let resolution = self.resolution_index(ticket)?;
        let old = self.resolutions[resolution].as_ref().ok_or(ClaimAdmissionError::Identity)?;
        if old.pending || old.failed {
            return Err(ClaimAdmissionError::Busy);
        }
        let proof = old.proof.as_ref().ok_or(ClaimAdmissionError::Busy)?.clone();
        let shared = Shared::new(Backend {
            resource,
            proof: proof.clone(),
            file: None,
            custody_probe: None,
            record: None,
            original_released: None,
            claimed: false,
            released: false,
            custodian: self.custodian.clone(),
        });
        let directory = lock(&self.root.value).directory.clone();
        let worker = shared.clone();
        let port = self.port.clone();
        let open = self.open.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(CLAIM_OPERATION_BYTES)
                    .map_err(|e| worker.fail(ClaimError::new("claim_scratch", e)))?;
                if context.cancellation_requested() || !open.load(Ordering::Acquire) {
                    return Ok(());
                }
                port.acquire(&mut lock(&worker.value), &directory, &context)
                    .map_err(|e| worker.fail(e))
            })
            .map_err(ClaimAdmissionError::Execution)?;
        self.resolutions[resolution] = None;
        self.backends[index] = Some(BackendEntry {
            resource,
            shared,
            proof,
            pending: true,
            claimed: false,
            prepared: Arc::new(AtomicU8::new(0)),
            launch: LaunchState::Unattempted,
            registry: None,
            failed: false,
        });
        self.pending[pending] = Some(Pending {
            job,
            action: Action::Claim(index),
            target: ClaimTarget::Backend(resource),
        });
        Ok(spec.id)
    }
}
pub(super) fn configuration_bytes(configuration: &LaunchConfiguration) -> u64 {
    use std::os::windows::ffi::OsStrExt;
    let paths = configuration.root().as_os_str().encode_wide().count()
        + configuration.interpreter().as_os_str().encode_wide().count();
    (paths * 3
        + std::mem::size_of_val(configuration.arguments())
        + configuration.arguments().iter().map(String::capacity).sum::<usize>()) as u64
}
