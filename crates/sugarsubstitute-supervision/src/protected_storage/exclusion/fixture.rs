// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain real execution claims while fake exclusion outputs remain externally owned.
use super::*;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{ExecutionId, JobContext, MemoryClaim, Moment, ScopeId};
pub(crate) trait ExclusionFixtureBoundary {
    fn adopt(
        &mut self,
        memory: Arc<MemoryClaim>,
        context: &JobContext,
        record_path: &Path,
        sid: &[u8],
    ) -> Result<[u8; 24], ClaimError>;
    fn close(&mut self, inode: [u8; 24], context: &JobContext) -> Result<(), ClaimError>;
    fn retire_unreturned(&mut self, context: &JobContext) -> Result<(), ClaimError>;
    fn is_closed(&self, inode: [u8; 24]) -> bool;
}
pub(super) struct FixtureReceipt {
    execution: ExecutionId,
    scope: ScopeId,
    deadline: Moment,
    inode: Option<[u8; 24]>,
    cleanup: Option<(ScopeId, Moment)>,
    _memory: Arc<MemoryClaim>,
}
impl<const N: usize> HeldFile<N> {
    /// Arm retained custody before the external boundary may create an output.
    pub(crate) fn adopt_fixture_into(
        retained: &mut Option<Self>,
        record_path: PathBuf,
        sid: &[u8],
        memory: Arc<MemoryClaim>,
        boundary: &mut dyn ExclusionFixtureBoundary,
        context: &JobContext,
    ) -> Result<[u8; 24], ClaimError> {
        let deadline = check(context)?;
        if retained.is_some() || !(16..=1024).contains(&N) || memory.bytes() == 0 {
            return Err(ClaimError::invalid("exclusion_fixture_admission"));
        }
        paths::wide(&record_path)?;
        let mut account_sid = Vec::new();
        account_sid
            .try_reserve_exact(sid.len())
            .map_err(|cause| ClaimError::new("fixture_sid", cause))?;
        account_sid.extend_from_slice(sid);
        *retained = Some(Self {
            handle: Handle::new(
                windows::Win32::Foundation::HANDLE::default(),
                Budget::Protected(memory.clone()),
            ),
            record_path: NativeName::Ordinary(record_path),
            exclusion_path: NativeName::Ordinary(PathBuf::new()),
            causes: NativeCauseDestinations::ordinary(),
            locked: false,
            created: false,
            account_sid: NativeSid::Ordinary(account_sid),
            temporary_handles: Mutex::new(Vec::new()),
            stage: Mutex::new(None),
            publication: Mutex::new(None),
            fixture_receipt: Some(FixtureReceipt {
                execution: context.owner_id(),
                scope: context.scope_id(),
                deadline,
                inode: None,
                cleanup: None,
                _memory: memory.clone(),
            }),
        });
        let file = retained
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("fixture_retained_exclusion_missing"))?;
        let inode =
            boundary.adopt(memory, context, file.record_path.path()?, file.account_sid.bytes()?)?;
        let receipt = file
            .fixture_receipt
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("fixture_receipt_missing"))?;
        receipt.inode = Some(inode);
        if inode == [0; 24]
            || context.owner_id() != receipt.execution
            || context.scope_id() != receipt.scope
            || check(context)? != receipt.deadline
        {
            return Err(ClaimError::invalid("fixture_adoption_changed_context"));
        }
        file.locked = true;
        Ok(inode)
    }
    /// Observe exact external close under the first latched original cleanup scope.
    pub(crate) fn retire_fixture(
        &mut self,
        inode: [u8; 24],
        boundary: &mut dyn ExclusionFixtureBoundary,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let deadline = check(context)?;
        let receipt = self
            .fixture_receipt
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("fixture_receipt_missing"))?;
        if receipt.execution != context.owner_id()
            || receipt.inode != Some(inode)
            || deadline
                .as_nanos()
                .checked_sub(context.now().as_nanos())
                .is_none_or(|remaining| remaining > 15_000_000_000)
        {
            return Err(ClaimError::invalid("fixture_retirement_context"));
        }
        if let Some((scope, until)) = receipt.cleanup
            && (scope != context.scope_id() || deadline > until)
        {
            return Err(ClaimError::invalid("fixture_cleanup_renewal"));
        }
        receipt.cleanup = Some((context.scope_id(), deadline));
        if lock(&self.stage).is_some() || !lock(&self.temporary_handles).is_empty() {
            return Err(ClaimError::invalid("fixture_live_record_custody"));
        }
        boundary.close(inode, context)?;
        if !boundary.is_closed(inode) {
            return Err(ClaimError::invalid("fixture_external_close_missing"));
        }
        check(context)?;
        self.locked = false;
        self.fixture_receipt = None;
        Ok(())
    }
}
fn check(context: &JobContext) -> Result<Moment, ClaimError> {
    let deadline =
        context.deadline().ok_or_else(|| ClaimError::invalid("fixture_deadline_missing"))?;
    if context.cancellation_requested() || context.now() >= deadline {
        return Err(ClaimError::invalid("fixture_context_expired"));
    }
    Ok(deadline)
}
