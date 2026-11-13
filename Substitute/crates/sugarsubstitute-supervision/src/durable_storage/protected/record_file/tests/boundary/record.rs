// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Model external fixed-file IO and writable sharing without replacing owner policy.
use super::*;
impl<const N: usize> RecordBoundary<N> for Fake {
    fn verify_exclusion(&mut self, file: &HeldFile<N>) -> Result<(), ClaimError> {
        let disk = lock(&self.0);
        let output =
            disk.exclusion.as_ref().ok_or_else(|| ClaimError::invalid("fake_exclusion_missing"))?;
        if !file.is_locked()
            || output.path != file.record_path.path()?
            || output.sid != file.account_sid.bytes()?
        {
            return Err(ClaimError::invalid("fake_exclusion_changed"));
        }
        Ok(())
    }
    fn read_shape(
        &mut self,
        _: &HeldFile<N>,
        expected: Option<[u8; 24]>,
        _check: &mut dyn FnMut() -> Result<(), ClaimError>,
        _operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<RecordRead<N>, ClaimError> {
        let mut disk = lock(&self.0);
        if disk.stage.as_ref().is_some_and(|stage| {
            stage.live
                && stage.path
                    == disk
                        .exclusion
                        .as_ref()
                        .map_or_else(PathBuf::new, |output| output.path.clone())
        }) {
            return Err(ClaimError::invalid("fake_writable_sharing_conflict"));
        }
        let Some(target) = disk.target.clone() else {
            return Ok(RecordRead::Missing);
        };
        if disk.reads.len() >= 16 {
            return Err(ClaimError::invalid("fake_read_capacity"));
        }
        let memory = disk
            .exclusion
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("fake_read_exclusion_missing"))?
            .memory
            .clone();
        disk.reads.push(memory);
        if disk.faults.read || expected.is_some_and(|identity| identity != disk.target_identity) {
            return Err(ClaimError::invalid("fake_changed_record"));
        }
        let observed = if target.is_empty() {
            RecordRead::Empty
        } else {
            RecordRead::Present(
                target.try_into().map_err(|_| ClaimError::invalid("fake_record_shape"))?,
            )
        };
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_read_close"));
        }
        disk.reads.pop();
        Ok(observed)
    }
    fn create_stage_into(
        &mut self,
        _: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        {
            let mut disk = lock(&self.0);
            let next = disk
                .next_stage
                .checked_add(1)
                .ok_or_else(|| ClaimError::invalid("fake_stage_identity_capacity"))?;
            let identity = [disk.next_stage; 24];
            disk.next_stage = next;
            let output = disk
                .exclusion
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("fake_stage_exclusion"))?;
            let path = output.path.with_extension("external.stage");
            let memory = output.memory.clone();
            value.path = path.clone();
            disk.stage = Some(ExternalStage {
                identity,
                path,
                bytes: Vec::new(),
                live: true,
                memory: Some(memory),
            });
        }
        self.after(Effect::Create)?;
        if lock(&self.0).faults.create {
            return Err(ClaimError::invalid("fake_partial_stage"));
        }
        Ok(())
    }
    fn reopen_stage_into(
        &mut self,
        _: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let memory = disk
            .exclusion
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("fake_reopen_exclusion"))?
            .memory
            .clone();
        let stage = disk.stage.as_mut().ok_or_else(|| ClaimError::invalid("fake_missing_stage"))?;
        if stage.path != value.path {
            return Err(ClaimError::invalid("fake_reopen_changed_path"));
        }
        stage.live = true;
        stage.memory = Some(memory);
        Ok(())
    }
    fn stage_is_open(&self, _: &Stage<N>) -> bool {
        lock(&self.0).stage.as_ref().is_some_and(|stage| stage.live)
    }
    fn verify_stage(&mut self, _: &HeldFile<N>, value: &mut Stage<N>) -> Result<(), ClaimError> {
        let disk = lock(&self.0);
        let stage = disk.stage.as_ref().ok_or_else(|| ClaimError::invalid("fake_stage_missing"))?;
        if disk.faults.changed_path
            || stage.path != value.path
            || value.identity.is_some_and(|identity| identity != stage.identity)
        {
            return Err(ClaimError::invalid("fake_stage_changed"));
        }
        value.identity = Some(stage.identity);
        Ok(())
    }
    fn write_stage(
        &mut self,
        _: &mut Stage<N>,
        encoded: &[u8; N],
        _check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        {
            let mut disk = lock(&self.0);
            let partial = disk.faults.write;
            let stage = disk
                .stage
                .as_mut()
                .ok_or_else(|| ClaimError::invalid("fake_write_missing_stage"))?;
            stage.bytes = if partial { encoded[..N / 2].to_vec() } else { encoded.to_vec() };
        }
        self.after(Effect::Write)?;
        if lock(&self.0).faults.write {
            return Err(ClaimError::invalid("fake_partial_write"));
        }
        Ok(())
    }
    fn activate_stage(
        &mut self,
        file: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        {
            let mut disk = lock(&self.0);
            let stage = disk
                .stage
                .as_mut()
                .ok_or_else(|| ClaimError::invalid("fake_rename_missing_stage"))?;
            stage.path = file.record_path.path()?.to_owned();
            let bytes = stage.bytes.clone();
            let identity = stage.identity;
            disk.target = Some(bytes);
            disk.target_identity = identity;
            value.path = file.record_path.path()?.to_owned();
            value.activated = Some(value.encoded);
        }
        self.after(Effect::Activate)?;
        if lock(&self.0).faults.activate {
            return Err(ClaimError::invalid("fake_post_rename"));
        }
        Ok(())
    }
    fn read_activated(
        &mut self,
        _: &mut Stage<N>,
        expected: &[u8; N],
        _check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        let disk = lock(&self.0);
        let stage = disk
            .stage
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("fake_retained_stage_missing"))?;
        if disk.faults.read || stage.bytes != expected {
            return Err(ClaimError::invalid("fake_changed_stage_bytes"));
        }
        Ok(())
    }
    fn flush_stage(&mut self, _: &mut Stage<N>) -> Result<(), ClaimError> {
        if lock(&self.0).faults.flush { Err(ClaimError::invalid("fake_flush")) } else { Ok(()) }
    }
    fn close_stage(&mut self, _: &mut Stage<N>) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_stage_close"));
        }
        if let Some(stage) = disk.stage.as_mut() {
            stage.live = false;
            stage.memory = None;
        }
        Ok(())
    }
    fn close_temporary(
        &mut self,
        _: &HeldFile<N>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
        _operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<(), ClaimError> {
        check()?;
        let mut disk = lock(&self.0);
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_temporary_close"));
        }
        disk.reads.clear();
        check()
    }
}
