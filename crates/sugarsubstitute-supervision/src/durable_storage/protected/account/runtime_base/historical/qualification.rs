// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualification-only stand-in for the primary account's earlier profile folder.
//!
//! Only builds that enable `qualification-historical-root` read the stand-in. It lets a headless
//! proof point a real binary at an owned synthetic profile instead of the account's real one, so
//! a proof never reads or touches the account's own data. Production builds do not contain it.
use super::*;
use std::{ffi::OsString, path::PathBuf};

/// Environment variable naming an absolute local folder that replaces `%LOCALAPPDATA%`.
const STAND_IN: &str = "SUGARSUBSTITUTE_QUALIFICATION_LOCALAPPDATA";

/// The stand-in folder when the proof supplied one; `None` selects the native answer.
pub(super) fn stand_in(phase: &mut NativePhase) -> Result<Option<PathBuf>, ClaimError> {
    from_value(std::env::var_os(STAND_IN)).map_err(|reason| phase.invalid(reason))
}

fn from_value(value: Option<OsString>) -> Result<Option<PathBuf>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("historical_stand_in_relative");
    }
    paths::validate_local_shape(&path).map_err(|_| "historical_stand_in_shape")?;
    crate::durable_storage::files::lexical(&path).map_err(|_| "historical_stand_in_lexical")?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_absolute_local_folder_replaces_the_native_answer() {
        assert_eq!(from_value(None), Ok(None));
        let folder = OsString::from(r"D:\proof\profile\AppData\Local");
        assert_eq!(
            from_value(Some(folder)),
            Ok(Some(PathBuf::from(r"D:\proof\profile\AppData\Local")))
        );
        for rejected in [r"AppData\Local", r"\\server\share\Local", r"D:\proof\..\other"] {
            assert!(from_value(Some(OsString::from(rejected))).is_err(), "{rejected}");
        }
    }
}
