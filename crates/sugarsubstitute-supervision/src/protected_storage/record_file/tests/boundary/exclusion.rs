// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the same real claim independently of HeldFile result and destructor lifetime.
use super::*;
fn check(context: &JobContext) -> Result<Moment, ClaimError> {
    let deadline =
        context.deadline().ok_or_else(|| ClaimError::invalid("fake_missing_deadline"))?;
    if context.cancellation_requested() || context.now() >= deadline {
        return Err(ClaimError::invalid("fake_expired_context"));
    }
    Ok(deadline)
}
fn cleanup(output: &mut Exclusion, context: &JobContext) -> Result<(), ClaimError> {
    let deadline = check(context)?;
    if output.execution != context.owner_id()
        || deadline
            .as_nanos()
            .checked_sub(context.now().as_nanos())
            .is_none_or(|remaining| remaining > 15_000_000_000)
    {
        return Err(ClaimError::invalid("fake_foreign_cleanup"));
    }
    if output.cleanup.is_some_and(|(scope, until)| scope != context.scope_id() || deadline > until)
    {
        return Err(ClaimError::invalid("fake_cleanup_renewal"));
    }
    output.cleanup = Some((context.scope_id(), deadline));
    Ok(())
}
impl ExclusionFixtureBoundary for Fake {
    fn adopt(
        &mut self,
        memory: Arc<MemoryClaim>,
        context: &JobContext,
        path: &Path,
        sid: &[u8],
    ) -> Result<[u8; 24], ClaimError> {
        check(context)?;
        let mut disk = lock(&self.0);
        if disk.exclusion.is_some() {
            return Err(ClaimError::invalid("fake_live_exclusion"));
        }
        let inode = [11; 24];
        disk.exclusion = Some(Exclusion {
            inode,
            path: path.to_path_buf(),
            sid: sid.to_vec(),
            memory,
            execution: context.owner_id(),
            cleanup: None,
            returned: false,
        });
        if disk.faults.adopt {
            return Err(ClaimError::invalid("fake_partial_exclusion"));
        }
        check(context)?;
        disk.exclusion
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("fake_exclusion_lost"))?
            .returned = true;
        Ok(inode)
    }
    fn close(&mut self, inode: [u8; 24], context: &JobContext) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        if disk.exclusion.is_none() && disk.closed == Some(inode) {
            check(context)?;
            return Ok(());
        }
        if disk.stage.as_ref().is_some_and(|stage| stage.live) || !disk.reads.is_empty() {
            return Err(ClaimError::invalid("fake_live_record_outputs"));
        }
        let output =
            disk.exclusion.as_mut().ok_or_else(|| ClaimError::invalid("fake_missing_exclusion"))?;
        if output.inode != inode || !output.returned {
            return Err(ClaimError::invalid("fake_changed_exclusion"));
        }
        cleanup(output, context)?;
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_close_exclusion"));
        }
        check(context)?;
        disk.closed = Some(inode);
        disk.exclusion = None;
        Ok(())
    }
    fn retire_unreturned(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let output = disk
            .exclusion
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("fake_missing_partial_exclusion"))?;
        if output.returned {
            return Err(ClaimError::invalid("fake_returned_exclusion"));
        }
        cleanup(output, context)?;
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_close_partial_exclusion"));
        }
        check(context)?;
        disk.closed = None;
        disk.exclusion = None;
        Ok(())
    }
    fn is_closed(&self, inode: [u8; 24]) -> bool {
        let disk = lock(&self.0);
        disk.exclusion.is_none() && disk.closed == Some(inode)
    }
}
