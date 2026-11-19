// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the names of the files an OLE drag carries.
//!
//! The page learns that a drag carries files but not what they are called until the drop, so
//! the host reads the drag's `CF_HDROP` list as the drag enters. Only each file's own name is
//! kept; the folders it lies in never leave this module.

use sugarsubstitute_contracts::host_file_drag::{MAX_DRAGGED_FILES, file_name};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{
        Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        Ole::ReleaseStgMedium,
    },
};

/// The predefined `CF_HDROP` clipboard format.
const CF_HDROP: u16 = 15;
/// `DROPFILES`: file list offset, drop point, nonclient flag and wide-character flag.
const DROPFILES_BYTES: usize = 20;
/// Most bytes of a file list read, far beyond what the named files need.
const MAX_LIST_BYTES: usize = 1 << 20;

/// The names of the files `data` carries, in its order; empty when it carries none.
pub(super) fn dragged_names(data: &IDataObject) -> Vec<String> {
    let format = FORMATETC {
        cfFormat: CF_HDROP,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    // SAFETY: OLE lends the data object for the current drag callback; the format outlives
    // the call and the returned medium is owned here until released below.
    let Ok(mut medium) = (unsafe { data.GetData(&raw const format) }) else {
        return Vec::new();
    };
    let names = if medium.tymed == TYMED_HGLOBAL.0 as u32 {
        // SAFETY: A medium of this kind holds a global block that stays valid until release.
        file_names(&copied(unsafe { medium.u.hGlobal }))
    } else {
        Vec::new()
    };
    // SAFETY: The receiver releases the medium exactly once.
    unsafe { ReleaseStgMedium(&raw mut medium) };
    names
}

/// Copy at most [`MAX_LIST_BYTES`] of a global block the caller keeps alive.
fn copied(global: HGLOBAL) -> Vec<u8> {
    // SAFETY: The caller owns the block for this call; it is locked only while copied, and
    // the copy never reads past the size the system reports.
    unsafe {
        let size = GlobalSize(global).min(MAX_LIST_BYTES);
        let source = GlobalLock(global);
        if source.is_null() {
            return Vec::new();
        }
        let bytes = std::slice::from_raw_parts(source.cast::<u8>(), size).to_vec();
        // A zero return with no error only reports that the block is now unlocked.
        let _ = GlobalUnlock(global);
        bytes
    }
}

/// Each listed file's own name from a `DROPFILES` block, at most [`MAX_DRAGGED_FILES`].
///
/// A list in the system's legacy code page is read only where it is plain ASCII, because its
/// other characters cannot be told apart here.
fn file_names(list: &[u8]) -> Vec<String> {
    let field = |at: usize| {
        list.get(at..at + 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let (Some(offset), Some(wide)) = (field(0), field(DROPFILES_BYTES - 4)) else {
        return Vec::new();
    };
    let Some(entries) = usize::try_from(offset).ok().and_then(|offset| list.get(offset..)) else {
        return Vec::new();
    };
    let paths: Vec<String> = if wide != 0 {
        let units: Vec<u16> =
            entries.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        units
            .split(|unit| *unit == 0)
            .take_while(|path| !path.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    } else {
        entries
            .split(|byte| *byte == 0)
            .take_while(|path| !path.is_empty())
            .filter(|path| path.is_ascii())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect()
    };
    paths
        .iter()
        .filter_map(|path| path.rsplit(['\\', '/']).next())
        .filter(|leaf| file_name(leaf))
        .take(MAX_DRAGGED_FILES)
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(wide: bool, paths: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for field in [DROPFILES_BYTES as u32, 0, 0, 0, u32::from(wide)] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        for path in paths {
            if wide {
                for unit in path.encode_utf16().chain([0]) {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            } else {
                bytes.extend_from_slice(path.as_bytes());
                bytes.push(0);
            }
        }
        bytes.extend_from_slice(if wide { &[0, 0] } else { &[0] });
        bytes
    }

    #[test]
    fn a_file_list_yields_each_files_own_name_in_order() {
        let listed = list(true, &[r"C:\Work\Portrait é.json", r"D:\shots\render.png"]);
        assert_eq!(file_names(&listed), ["Portrait é.json", "render.png"]);
        assert_eq!(file_names(&list(false, &[r"C:\Work\a.json"])), ["a.json"]);
    }

    #[test]
    fn only_the_first_sixteen_files_are_named() {
        let paths: Vec<String> = (0..20).map(|index| format!(r"C:\w\{index}.json")).collect();
        let borrowed: Vec<&str> = paths.iter().map(String::as_str).collect();
        let names = file_names(&list(true, &borrowed));
        assert_eq!(names.len(), MAX_DRAGGED_FILES);
        assert_eq!(names[15], "15.json");
    }

    #[test]
    fn malformed_and_unnameable_lists_yield_nothing() {
        assert!(file_names(&[]).is_empty());
        assert!(file_names(&[0; DROPFILES_BYTES - 1]).is_empty());
        let mut beyond = list(true, &[r"C:\a.json"]);
        beyond[0..4].copy_from_slice(&9999_u32.to_le_bytes());
        assert!(file_names(&beyond).is_empty());
        // A drive root has no file name, and a legacy list is read only where it is ASCII.
        assert!(file_names(&list(true, &[r"C:\"])).is_empty());
        let mut legacy = list(false, &[r"C:\a.json"]);
        legacy[DROPFILES_BYTES + 3] = 0xe9;
        assert!(file_names(&legacy).is_empty());
    }
}
