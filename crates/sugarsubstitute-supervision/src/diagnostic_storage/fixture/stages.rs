// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe cached external stage names separately from real recovery admission.
use super::*;
impl Filesystem {
    #[cfg(test)]
    pub(in crate::diagnostic_storage) fn put_stage(
        &self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        bytes: &[u8],
    ) -> DiagnosticReadFacts {
        let facts = self.put(object, bytes);
        let mut disk = lock(&self.0);
        if let Some(file) = disk.files.last_mut() {
            file.active = false;
            file.historical = Some(historical);
        }
        facts
    }

    #[cfg(test)]
    pub(in crate::diagnostic_storage) fn change_stage(&self, fault: u8) {
        let mut disk = lock(&self.0);
        let Some(index) =
            disk.files.iter().position(|file| file.historical.is_some() && !file.active)
        else {
            return;
        };
        let file = &mut disk.files[index];
        match fault {
            0 => file.identity = [0; 24],
            1 => {
                if let Some(byte) = file.bytes.first_mut() {
                    *byte ^= 1;
                }
            }
            2 => file.bytes.push(1),
            3 => file.identity = [8; 24],
            4 => {
                if let Some(historical) = file.historical {
                    file.object = DiagnosticObject::Journal(historical);
                }
            }
            _ => (),
        }
    }
    pub(in crate::diagnostic_storage) fn recover_existing(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        _: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let mut disk = lock(&self.0);
        if !disk.stage_scan_complete {
            return Err(ClaimError::invalid("fake_incomplete_stage_scan"));
        }
        let mut matching = disk.stage_scan.iter().filter(|(_, target, origin, identity, bytes)| {
            *target == object
                && *origin == historical
                && *identity == expected.file_identity()
                && *bytes == expected.bytes()
        });
        let file = matching.next().ok_or_else(|| ClaimError::invalid("fake_missing_stage"))?.0;
        if matching.next().is_some() {
            return Err(ClaimError::invalid("fake_ambiguous_stage"));
        }
        if disk.files[file].identity == [0; 24]
            || disk.files[file].object != object
            || disk.files[file].historical != Some(historical)
        {
            return Err(ClaimError::invalid("fake_missing_stage_path"));
        }
        let id = handle(&mut disk, file)?;
        let actual = facts(&disk.files[file]);
        if let Some(clock) = &disk.after_open {
            clock.observe(disk.after_open_time);
        }
        if disk.faults.partial_open || !expected.matches(actual) {
            return Err(ClaimError::invalid("fake_stage_after_adoption"));
        }
        Ok((id, actual))
    }
}
