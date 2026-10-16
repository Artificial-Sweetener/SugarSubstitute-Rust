// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain publication outputs before provider effects and checked-close in ownership order.
use super::*;
pub(super) struct Publication {
    repository: Option<IncidentRepository>,
    opening: Option<DiagnosticStoreOpening>,
    output: Option<RecoveryCaptureOriginCommit>,
    published: Option<Published>,
    original: Option<Arc<failure::FailureData>>,
    cleanup: Option<Arc<failure::FailureData>>,
    started: bool,
    retired: bool,
}
impl Publication {
    pub(super) const fn new() -> Self {
        Self {
            repository: None,
            opening: None,
            output: None,
            published: None,
            original: None,
            cleanup: None,
            started: false,
            retired: false,
        }
    }
    /// The real8MiB context funds storage before any native namespace or publication effect.
    pub(super) fn execute(
        &mut self,
        input: &Input,
        context: &JobContext,
    ) -> Result<(), OriginIssue> {
        if self.started {
            return Err(OriginIssue::Started);
        }
        self.started = true;
        if let Err(cause) = input.grant.check_context(context) {
            self.original = Some(Arc::new(failure::FailureData::grant(cause)));
            return Err(OriginIssue::Operation);
        }
        if context.spec().operation != Operation::NativeLifecycle
            || context.spec().class != JobClass::InfrastructureLifecycle
            || context.spec().reservation_bytes != DIAGNOSTIC_STORAGE_BYTES
            || context.spec().deadline != Some(input.grant.original_budget().until())
        {
            return Err(OriginIssue::Identity);
        }
        let claim = match context.memory().claim(DIAGNOSTIC_STORAGE_BYTES) {
            Ok(claim) => claim,
            Err(cause) => {
                self.original = Some(Arc::new(failure::FailureData::memory(cause)));
                return Err(OriginIssue::Operation);
            }
        };
        let result = (|| {
            let mut storage = DiagnosticStorage::new(claim)?;
            let operation = storage.begin_operation(input.operation, context)?;
            self.repository = Some(IncidentRepository::new(storage, operation, context)?);
            let repository =
                self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            self.opening = Some(repository.resolve_namespace(&input.request, context)?);
            repository.acquire_writer(context)?;
            match self.opening.as_mut().ok_or(IncidentStoreError::RecoveryConflict)? {
                DiagnosticStoreOpening::Created(genesis) => {
                    repository.initialize(genesis, context)?;
                }
                DiagnosticStoreOpening::Existing => {
                    repository.validate_store(context)?;
                }
            }
            repository.publish_recovery_capture_origin(
                &input.grant,
                &input.capture,
                input.at,
                &mut self.output,
                context,
            )?;
            let commit = self.output.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
            let checked = commit.checked()?;
            let origin = RecoveryCaptureIdentity::from_grant(input.grant.key());
            if commit.origin() != origin
                || commit.operation() != input.operation
                || checked.origin() != origin
                || checked.current_operation() != input.operation
                || checked.historical_operation() != input.operation
                || Some(checked.root_identity()) != input.capture.root_identity()
            {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            self.published = Some(Published {
                key: input.grant.key(),
                operation: input.operation,
                origin,
                root_identity: checked.root_identity(),
                journal: checked.journal_facts(),
            });
            input.grant.check_context(context).map_err(|cause| IncidentStoreError::Storage(
                sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError::from_recovery_grant(cause)))?;
            Ok::<_, IncidentStoreError>(())
        })();
        if let Err(cause) = result {
            self.original = Some(Arc::new(failure::FailureData::store(
                OriginFailureCategory::Publication,
                cause,
            )));
            return Err(OriginIssue::Operation);
        }
        Ok(())
    }
    /// Retire the repository's complete native read/file custody through its checked cleanup owner.
    /// The origin output keeps its shared read pin and funding until successful source compaction.
    pub(super) fn retire(&mut self, context: &JobContext) -> Result<bool, OriginIssue> {
        if self.retired {
            return Ok(true);
        }
        if let Some(repository) = &mut self.repository
            && let Err(cause) = repository.retire(context)
        {
            self.cleanup.get_or_insert_with(|| {
                Arc::new(failure::FailureData::store(
                    OriginFailureCategory::RepositoryCleanup,
                    cause,
                ))
            });
            return Err(OriginIssue::Cleanup);
        }
        self.retired = true;
        Ok(true)
    }
    pub(super) const fn retired(&self) -> bool {
        self.retired
    }
    pub(super) const fn releasable(&self) -> bool {
        !self.started || self.retired
    }
    pub(super) fn completed(&self) -> Option<Published> {
        if self.retired && self.original.is_none() && self.cleanup.is_none() {
            self.published
        } else {
            None
        }
    }
    pub(super) fn failure(&self, cleanup: bool) -> Option<Arc<failure::FailureData>> {
        if cleanup { self.cleanup.clone() } else { self.original.clone() }
    }
}

#[cfg(test)]
#[path = "operation/tests.rs"]
pub(super) mod tests;
