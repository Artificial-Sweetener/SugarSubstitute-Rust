// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit cleanup from retained native ownership proofs, never a caller's settled flag.
use super::*;
use crate::{FamilyBinding, WindowsProcessRegistry};
impl WindowsResourceClaims {
    /// Close an unclaimed resolution on a finite worker after actual resolution completion.
    pub fn submit_retire_resolution(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        ticket: &ResolutionTicket,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.validate_scope(scope)?;
        let pending = self.admit_index(spec, false)?;
        let index = self.resolution_index(ticket)?;
        let entry = self.resolutions[index].as_mut().ok_or(ClaimAdmissionError::Identity)?;
        if entry.pending {
            return Err(ClaimAdmissionError::Busy);
        }
        let proof = entry.proof.as_ref().ok_or(ClaimAdmissionError::Busy)?.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(CLAIM_OPERATION_BYTES)
                    .map_err(|e| ClaimError::new("retire_scratch", e))?;
                proof.close()
            })
            .map_err(ClaimAdmissionError::Execution)?;
        entry.pending = true;
        self.pending[pending] = Some(Pending {
            job,
            action: Action::Retire(index),
            target: ClaimTarget::Resolution(ticket.clone()),
        });
        Ok(spec.id)
    }
    /// Release exclusion only after the exact accepted family's settlement or exact rejection retirement.
    /// Accepted families must remain in the registry until this release observation is consumed.
    pub fn submit_backend_release(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        registry: &WindowsProcessRegistry,
        resource: ResourceKey,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.validate_scope(scope)?;
        let pending = self.admit_index(spec, false)?;
        let index = self.backend_index(resource)?;
        let identity =
            registry.claim_identity(resource.owner).map_err(ClaimAdmissionError::Registry)?;
        if !self.registry.as_ref().is_some_and(|bound| bound.same_as(&identity)) {
            return Err(ClaimAdmissionError::Identity);
        }
        let entry = self.backends[index].as_mut().ok_or(ClaimAdmissionError::Identity)?;
        if entry.pending || entry.prepared.load(Ordering::Acquire) == 1 {
            return Err(ClaimAdmissionError::Busy);
        }
        match entry.launch {
            LaunchState::Unattempted => (),
            LaunchState::Accepted(ticket) => {
                if !entry.registry.as_ref().is_some_and(|id| id.same_as(&identity)) {
                    return Err(ClaimAdmissionError::Identity);
                }
                let snapshot = registry
                    .retained_managed(resource)
                    .map_err(ClaimAdmissionError::Registry)?
                    .ok_or(ClaimAdmissionError::Identity)?;
                if snapshot.process != ticket.process()
                    || snapshot.binding != FamilyBinding::Managed(resource)
                {
                    return Err(ClaimAdmissionError::Identity);
                }
                if !snapshot.settled {
                    return Err(ClaimAdmissionError::Busy);
                }
            }
            LaunchState::Rejected => {
                if !entry.registry.as_ref().is_some_and(|id| id.same_as(&identity)) {
                    return Err(ClaimAdmissionError::Identity);
                }
                if registry
                    .retained_managed(resource)
                    .map_err(ClaimAdmissionError::Registry)?
                    .is_some()
                {
                    return Err(ClaimAdmissionError::Busy);
                }
            }
        }
        let shared = entry.shared.clone();
        let port = self.port.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(CLAIM_OPERATION_BYTES)
                    .map_err(|e| shared.fail(ClaimError::new("release_scratch", e)))?;
                port.release(&mut lock(&shared.value)).map_err(|e| shared.fail(e))
            })
            .map_err(ClaimAdmissionError::Execution)?;
        entry.pending = true;
        self.pending[pending] = Some(Pending {
            job,
            action: Action::Release(index),
            target: ClaimTarget::Backend(resource),
        });
        Ok(spec.id)
    }
    /// Close only this adapter's storage after its retained resolutions/backends and finite jobs retire.
    /// Call before shutting down the executor, using a live cleanup scope. No IPC/profile settlement is implied.
    pub fn submit_close(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        registry: &mut WindowsProcessRegistry,
    ) -> Result<JobId, ClaimAdmissionError> {
        if self.open.load(Ordering::Acquire) || self.closed || self.initialization_pending {
            return Err(ClaimAdmissionError::Closed);
        }
        self.validate_scope(scope)?;
        if self.pending.iter().any(Option::is_some)
            || self.resolutions.iter().any(Option::is_some)
            || self.backends.iter().any(Option::is_some)
        {
            return Err(ClaimAdmissionError::Busy);
        }
        if let Some(owner) = self.owner {
            let identity = registry.claim_identity(owner).map_err(ClaimAdmissionError::Registry)?;
            if !self.registry.as_ref().is_some_and(|id| id.same_as(&identity)) {
                return Err(ClaimAdmissionError::Identity);
            }
            registry.close_launch_admission();
            if !registry.is_empty() {
                return Err(ClaimAdmissionError::Busy);
            }
        }
        let pending = self.admit_index(spec, false)?;
        let shared = self.root.clone();
        let port = self.port.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let _scratch = context
                    .memory()
                    .claim(CLAIM_OPERATION_BYTES)
                    .map_err(|e| shared.fail(ClaimError::new("close_scratch", e)))?;
                port.close(&mut lock(&shared.value)).map_err(|e| shared.fail(e))
            })
            .map_err(ClaimAdmissionError::Execution)?;
        self.pending[pending] =
            Some(Pending { job, action: Action::Close, target: ClaimTarget::Coordinator });
        Ok(spec.id)
    }
}
