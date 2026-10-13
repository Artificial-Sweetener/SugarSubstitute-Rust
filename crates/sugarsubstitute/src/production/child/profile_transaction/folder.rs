// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain an exact Windows request path under the request's already admitted reservation.
use super::ProfileError;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::execution::MemoryClaim;
use sugarsubstitute_desktop::MainWindowRequest;
use sugarsubstitute_supervision::lifecycle_protocol::MAX_PROFILE_FOLDER_UNITS;
pub(super) struct RequestedFolder {
    pub(super) units: Vec<u16>,
    _claim: MemoryClaim,
}
impl RequestedFolder {
    pub(super) fn new(request: &MainWindowRequest) -> Result<Self, ProfileError> {
        let count = request.profile_directory().as_os_str().encode_wide().count();
        if !(1..=MAX_PROFILE_FOLDER_UNITS).contains(&count) {
            return Err(ProfileError::Folder);
        }
        let claim = request
            .storage_lease()
            .claim((count * size_of::<u16>() + size_of::<Self>()) as u64)
            .map_err(ProfileError::Claim)?;
        let mut units = Vec::new();
        units.try_reserve_exact(count).map_err(ProfileError::Allocation)?;
        if units.capacity() != count {
            return Err(ProfileError::Folder);
        }
        units.extend(request.profile_directory().as_os_str().encode_wide());
        if units.contains(&0) {
            return Err(ProfileError::Folder);
        }
        Ok(Self { units, _claim: claim })
    }
}
