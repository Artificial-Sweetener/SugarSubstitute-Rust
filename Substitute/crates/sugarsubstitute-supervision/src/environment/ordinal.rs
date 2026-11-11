// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Isolate synchronous Windows ordinal comparison; no pointers escape this call.
//! Windows uses its uppercase table, independently of the user's locale.
//! See [CompareStringOrdinal](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal).
use super::{LaunchEnvironmentError, MAX_UNITS};
use std::{cmp::Ordering, io};
use windows::Win32::Globalization::CompareStringOrdinal;

pub(super) fn compare(left: &[u16], right: &[u16]) -> Result<Ordering, LaunchEnvironmentError> {
    if left.is_empty() || right.is_empty() || left.len() > MAX_UNITS || right.len() > MAX_UNITS {
        return Err(LaunchEnvironmentError::InvalidName);
    }
    // SAFETY: borrowed contiguous buffers stay live for this synchronous call;
    // explicit positive lengths <=32767 fit i32, and no NUL-termination is assumed.
    // The native function retains neither pointer and requires no thread affinity.
    let result = unsafe { CompareStringOrdinal(left, right, true) }.0;
    if result == 0 {
        return decode(result, io::Error::last_os_error());
    }
    decode(result, io::Error::from(io::ErrorKind::InvalidData))
}
fn decode(result: i32, cause: io::Error) -> Result<Ordering, LaunchEnvironmentError> {
    match result {
        1 => Ok(Ordering::Less),
        2 => Ok(Ordering::Equal),
        3 => Ok(Ordering::Greater),
        _ => Err(LaunchEnvironmentError::Ordinal(cause)),
    }
}
pub(super) fn private_name(name: &[u16]) -> Result<bool, LaunchEnvironmentError> {
    for prefix in [b"SUGAR_SUBSTITUTE_".as_slice(), b"SUGARSUBSTITUTE_", b"SUBSTITUTE_"] {
        if name.len() >= prefix.len() {
            let mut wide = [0; 17];
            for (target, byte) in wide.iter_mut().zip(prefix) {
                *target = u16::from(*byte);
            }
            if compare(&name[..prefix.len()], &wide[..prefix.len()])? == Ordering::Equal {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    #[test]
    fn native_failure_never_becomes_equal_and_preserves_cause() -> Result<(), Box<dyn Error>> {
        let Err(error) = decode(0, io::Error::from_raw_os_error(87)) else {
            return Err("failed native comparison must reject".into());
        };
        assert_eq!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<io::Error>())
                .and_then(io::Error::raw_os_error),
            Some(87)
        );
        assert!(matches!(
            decode(4, io::Error::from_raw_os_error(13)),
            Err(LaunchEnvironmentError::Ordinal(_))
        ));
        Ok(())
    }
}
