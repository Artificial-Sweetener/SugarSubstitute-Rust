// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold setup's installation mutation exclusion independently of the application lease.
use super::*;
use crate::setup_storage::ownership;

/// Filesystem custody and process-lifetime exclusion for one installation mutation.
pub(in crate::setup_storage) struct Admission {
    pub(in crate::setup_storage) root: Directory,
    _marker: File,
    _lock: files::FileExclusion,
}
impl Admission {
    /// Adopt only an empty directory, or one holding just the default setup workspace, when
    /// requested; otherwise require verified ownership.
    /// Original job controls cover empty-folder inspection and exclusion acquisition.
    pub(in crate::setup_storage) fn open(
        path: &Path,
        create: bool,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let root = Directory::open(path, create)?;
        if create && !ownership::present(&root)? {
            if !ownership::adoptable(&root, context)? {
                return Err(invalid("This folder contains files from an unknown installation"));
            }
            ownership::establish(&root)?;
        }
        let marker = ownership::read(&root)?;
        let lock = ownership::state(&root)?.lock(super::SETUP_LOCK, context)?;
        check(context)?;
        Ok(Self { root, _marker: marker, _lock: lock })
    }
}
