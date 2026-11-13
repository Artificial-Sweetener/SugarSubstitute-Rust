// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Obtain the actual loaded executable name through the original capture owner.
use super::{
    allocation::AllocationOrigin,
    names::{CaptureNativeName, CapturePathRole},
};
use crate::{
    ClaimError, claim_error::BoundedNativeCause,
    durable_storage::protected::native_phase::NativePhase,
};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
/// Keep the NUL output and conversion within the admitted exclusive scratch phase.
pub(super) fn read(
    original: &AllocationOrigin,
    phase: &mut NativePhase<'_>,
) -> Result<CaptureNativeName, ClaimError> {
    let destination =
        original.name(CapturePathRole::CurrentImage).map_err(ClaimError::capture_allocation)?;
    phase.path()?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(32767)
        .map_err(|c| phase.error("current_image_units", BoundedNativeCause::Reserve(c)))?;
    if units.capacity() > 32767 {
        return Err(phase.invalid("current_image_units_capacity"));
    }
    units.resize(32767, 0);
    phase.check()?;
    // SAFETY: The actual current module and exact initialized native output remain
    // borrowed through this finite call; zero/truncated outputs never select a path.
    let count = unsafe { GetModuleFileNameW(None, &mut units) } as usize;
    if count == 0 {
        return Err(phase
            .error("current_image", BoundedNativeCause::RawIo(std::io::Error::last_os_error())));
    }
    if count >= units.len() || units[..count].contains(&0) {
        return Err(phase.invalid("current_image_bound"));
    }
    let decoded = OsString::from_wide(&units[..count]);
    drop(units);
    let mut path = OsString::new();
    path.try_reserve_exact(super::allocation::PATH_BYTES)
        .map_err(|c| phase.error("current_image_path", BoundedNativeCause::Reserve(c)))?;
    if path.capacity() > super::allocation::PATH_BYTES {
        return Err(phase.invalid("current_image_capacity"));
    }
    let parsed = std::path::Path::new(&decoded);
    append_current_name(&mut path, parsed);
    drop(decoded);
    let path = PathBuf::from(path);
    crate::durable_storage::protected::paths::validate_local_shape(&path)
        .map_err(|reason| phase.invalid(reason))?;
    phase.check()?;
    destination.finish(path).map_err(|cause| phase.selected_name_error(cause))
}

/// Append a DOS prefix as raw OS text; PathBuf::push would replace an absolute prefix.
fn append_current_name(output: &mut OsString, current: &std::path::Path) {
    if matches!(current.components().next(),Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(),std::path::Prefix::Disk(_)))
    {
        output.push(r"\\?\");
    }
    output.push(current.as_os_str());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_dos_name_gets_complete_verbatim_prefix_without_replacing_absolute_drive() {
        let mut output = OsString::new();
        append_current_name(&mut output, std::path::Path::new(r"Q:\owned\sugarsubstitute.exe"));
        assert_eq!(output, OsString::from(r"\\?\Q:\owned\sugarsubstitute.exe"));
    }
    #[test]
    fn already_verbatim_current_name_preserves_raw_unpaired_utf16_units() {
        use std::os::windows::ffi::OsStrExt;
        let mut units = r"\\?\Q:\owned\".encode_utf16().collect::<Vec<_>>();
        units.push(0xd800);
        units.extend(".exe".encode_utf16());
        let name = OsString::from_wide(&units);
        let mut output = OsString::new();
        append_current_name(&mut output, std::path::Path::new(&name));
        assert_eq!(output.encode_wide().collect::<Vec<_>>(), units);
    }
}
