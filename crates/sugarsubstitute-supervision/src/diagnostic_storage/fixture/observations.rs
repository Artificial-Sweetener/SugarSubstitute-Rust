// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound externally observed fixture files and expose categorical snapshots.
use super::*;
impl Filesystem {
    pub(in crate::diagnostic_storage) fn reserve(&self) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        disk.files
            .try_reserve_exact(DIAGNOSTIC_SCAN_BOUND)
            .and_then(|()| disk.handles.try_reserve_exact(DIAGNOSTIC_HANDLE_BOUND))
            .map_err(|cause| ClaimError::new("fake_observation_capacity", cause))
    }
    pub(in crate::diagnostic_storage) fn put_checked(
        &self,
        object: DiagnosticObject,
        historical: Option<LogicalStoreOperationKey>,
        bytes: &[u8],
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        if bytes.len() as u64 > object.bound()
            || historical.is_some_and(|key| {
                object.namespace().is_some_and(|namespace| namespace != key.owner().namespace)
            })
        {
            return Err(ClaimError::invalid("fake_object_bound"));
        }
        let mut disk = lock(&self.0);
        if disk.files.len() >= DIAGNOSTIC_SCAN_BOUND {
            return Err(ClaimError::invalid("fake_file_capacity"));
        }
        let total = disk
            .files
            .iter()
            .filter(|file| file.identity != [0; 24])
            .try_fold(bytes.len() as u64, |sum, file| sum.checked_add(file.bytes.len() as u64));
        if total.is_none_or(|value| value > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES)
        {
            return Err(ClaimError::invalid("fake_physical_capacity"));
        }
        if disk.handles.iter().any(|handle| {
            handle.live
                && disk.files[handle.file].object == object
                && disk.files[handle.file].active
                && historical.is_none()
        }) {
            return Err(ClaimError::invalid("fake_retained_inode"));
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|cause| ClaimError::new("fake_observed_bytes", cause))?;
        owned.extend_from_slice(bytes);
        if historical.is_none() {
            for file in &mut disk.files {
                if file.object == object && file.active {
                    file.identity = [0; 24];
                }
            }
        }
        let identity = identity(disk.files.len());
        disk.files.push(File {
            object,
            bytes: owned,
            identity,
            active: historical.is_none(),
            historical,
        });
        Ok(facts(&disk.files[disk.files.len() - 1]))
    }
    pub(in crate::diagnostic_storage) fn erase_checked(
        &self,
        object: DiagnosticObject,
    ) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        if disk.handles.iter().any(|handle| handle.live && disk.files[handle.file].object == object)
        {
            return Err(ClaimError::invalid("fake_retained_inode"));
        }
        for file in &mut disk.files {
            if file.object == object {
                file.identity = [0; 24];
            }
        }
        Ok(())
    }
    pub(in crate::diagnostic_storage) fn set_failure(
        &self,
        failure: DiagnosticFixtureFailure,
        enabled: bool,
    ) {
        let mut disk = lock(&self.0);
        *match failure {
            DiagnosticFixtureFailure::PartialCreate => &mut disk.faults.partial_stage,
            DiagnosticFixtureFailure::PartialOpen => &mut disk.faults.partial_open,
            DiagnosticFixtureFailure::ReadCorruption => &mut disk.faults.corrupt_read,
            DiagnosticFixtureFailure::Close => &mut disk.faults.close,
            DiagnosticFixtureFailure::PostActivation => &mut disk.faults.activate,
            DiagnosticFixtureFailure::AbsenceObservation => &mut disk.faults.absence,
            DiagnosticFixtureFailure::IncompleteScan => &mut disk.faults.scan_incomplete,
            DiagnosticFixtureFailure::ChangedInventory => &mut disk.faults.changed_inventory,
        } = enabled;
    }
    pub(in crate::diagnostic_storage) fn snapshot(&self) -> DiagnosticFixtureSnapshot {
        let disk = lock(&self.0);
        DiagnosticFixtureSnapshot {
            live_outputs: disk.handles.iter().filter(|handle| handle.live).count(),
            mutation_count: disk.mutations,
            active: disk
                .files
                .iter()
                .filter(|file| file.active && file.identity != [0; 24])
                .map(|file| (file.object, facts(file)))
                .collect(),
        }
    }
}
