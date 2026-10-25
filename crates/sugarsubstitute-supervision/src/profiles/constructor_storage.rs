// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Account the profile owner's genuine lexical allocations before native admission.
use super::*;
use std::{os::windows::ffi::OsStrExt, path::Path};

const fn arc<T>() -> u64 {
    let alignment =
        if align_of::<T>() > align_of::<usize>() { align_of::<T>() } else { align_of::<usize>() };
    (2 * size_of::<usize>() + 2 * (alignment - 1) + size_of::<T>()) as u64
}
impl WindowsProfileLease {
    /// Requested layouts allocated by lexical construction, including temporary
    /// native-path conversion. Inline Self and the separately supplied PathBuf
    /// allocation are excluded; the caller funds those exact actual capacities.
    /// Native acquisition, paths/records prepared later and OS allocator internals
    /// remain under their existing genuine operation reservations.
    ///
    /// This readonly query allocates nothing and grants no registry/native access.
    /// # Errors
    /// Returns Specification for an invalid local shape or checked-size overflow.
    pub fn constructor_storage_bytes(path: &Path) -> Result<u64, ClaimAdmissionError> {
        crate::durable_storage::protected::paths::validate_local_shape(path)
            .map_err(|_| ClaimAdmissionError::Specification)?;
        let units = u64::try_from(path.as_os_str().encode_wide().count())
            .map_err(|_| ClaimAdmissionError::Specification)?;
        let fixed = (size_of::<storage::NativeState>() as u64)
            .checked_add(arc::<Shared>())
            .and_then(|bytes| bytes.checked_add(2 * arc::<AtomicU8>()))
            .and_then(|bytes| bytes.checked_add(arc::<AtomicU64>()))
            .and_then(|bytes| bytes.checked_add(arc::<storage::WindowsProfilePort>()))
            .ok_or(ClaimAdmissionError::Specification)?;
        // collect + trailing NUL can reallocate: pinned RawVec doubling/min-eight,
        // with old and new requested layouts both present at peak.
        let scratch = units
            .checked_add(1)
            .and_then(|units| units.checked_mul(2))
            .and_then(|units| units.checked_add(8))
            .and_then(|units| units.checked_mul(2))
            .and_then(|units| units.checked_mul(size_of::<u16>() as u64))
            .ok_or(ClaimAdmissionError::Specification)?;
        fixed.checked_add(scratch).ok_or(ClaimAdmissionError::Specification)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accounting_grows_with_real_local_units_and_never_accepts_nonlocal_shape()
    -> Result<(), ClaimAdmissionError> {
        let short = WindowsProfileLease::constructor_storage_bytes(Path::new(r"E:\profile"))?;
        let long = PathBuf::from(format!(r"E:\{}", "p".repeat(2048)));
        let larger = WindowsProfileLease::constructor_storage_bytes(&long)?;
        assert!(larger > short);
        assert!(short >= size_of::<storage::NativeState>() as u64);
        assert!(matches!(
            WindowsProfileLease::constructor_storage_bytes(Path::new("relative")),
            Err(ClaimAdmissionError::Specification)
        ));
        Ok(())
    }
    #[test]
    fn excessive_or_nul_path_is_rejected_before_constructor_allocation() {
        let long = PathBuf::from(format!(r"E:\{}", "x".repeat(32767)));
        assert!(matches!(
            WindowsProfileLease::constructor_storage_bytes(&long),
            Err(ClaimAdmissionError::Specification)
        ));
        let nul = PathBuf::from("E:\\a\0b");
        assert!(matches!(
            WindowsProfileLease::constructor_storage_bytes(&nul),
            Err(ClaimAdmissionError::Specification)
        ));
    }
}
