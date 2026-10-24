// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold setup's installation mutation exclusion independently of the application lease.
use super::*;
use crate::setup_storage::ownership;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

/// Filesystem custody and process-lifetime exclusion for one installation mutation.
pub(in crate::setup_storage) struct Admission {
    pub(in crate::setup_storage) root: Directory,
    _marker: File,
    _lock: File,
}
impl Admission {
    /// Adopt only an empty directory when requested; otherwise require verified ownership.
    /// Original job controls cover empty-folder inspection and exclusion acquisition.
    pub(in crate::setup_storage) fn open(
        path: &Path,
        create: bool,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let root = Directory::open(path, create)?;
        let marker_path = root.path().join(ownership::MARKER);
        if create && !marker_path.try_exists()? {
            if root.entries(context)?.next().transpose()?.is_some() {
                return Err(invalid("This folder contains files from an unknown installation"));
            }
            files::write_new(&marker_path, ownership::CONTENT)?;
        }
        let marker = ownership::read(&root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(3)
            .custom_flags(0x0020_0000)
            .open(root.path().join(".setup-lock"))?;
        let metadata = lock.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("Installation exclusion must be a regular file"));
        }
        lock.try_lock().map_err(io::Error::from)?;
        check(context)?;
        Ok(Self { root, _marker: marker, _lock: lock })
    }
}
