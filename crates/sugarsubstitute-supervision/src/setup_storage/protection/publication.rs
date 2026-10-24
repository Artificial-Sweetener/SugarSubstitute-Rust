// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a completed protected filesystem object through its retained native handle.
use super::*;
use crate::{protected_storage::native_rename, setup_storage::files};
use std::{
    fs::OpenOptions,
    os::windows::{fs::OpenOptionsExt, io::IntoRawHandle},
};

/// Select publication semantics after the caller verifies ownership of any replaced file.
pub(in crate::setup_storage) enum Publication {
    /// Publish a new directory and retain its native pin through the protection operation.
    Directory,
    /// Publish a regular file and release rename access before callers reopen it for reading.
    File { replace: bool },
}

impl Protection {
    /// Reopen the staged object with rename access, verify its original identity and publish it.
    /// Earlier source pins must retire before opening rename access. The common parent stays
    /// pinned, including capture owners that deny write sharing on that directory.
    /// Directory publication never replaces an existing destination. File replacement requires
    /// the caller's verified ownership and continuously held installation mutation exclusion.
    /// Native ownership and any failed-close custody stay with this protection operation.
    pub(in crate::setup_storage) fn publish(
        &mut self,
        source: &Path,
        destination: &Path,
        expected: (u32, u32, u32),
        publication: Publication,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        files::lexical(source)?;
        files::lexical(destination)?;
        if source == destination || source.parent() != destination.parent() {
            return Err(super::super::invalid("Publication requires distinct siblings"));
        }
        let _parent = Directory::open(
            source.parent().ok_or_else(|| super::super::invalid("Publication parent missing"))?,
            false,
        )?;
        if self.handles.len() == self.handles.capacity() {
            return Err(super::super::invalid("Publication handle reservation exhausted"));
        }
        let directory = matches!(publication, Publication::Directory);
        let file = OpenOptions::new()
            .access_mode((DELETE | FILE_READ_ATTRIBUTES | READ_CONTROL | SYNCHRONIZE).0)
            .share_mode(if directory { (FILE_SHARE_READ | FILE_SHARE_WRITE).0 } else { 0 })
            .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
            .open(source)?;
        let index = self.handles.len();
        self.handles.push(Handle::new(HANDLE(file.into_raw_handle()), self.budget.clone()));
        let mut phase = NativePhase::ordinary(&self.budget).map_err(io::Error::other)?;
        {
            let file = self.handles[index].borrow_file().map_err(io::Error::other)?;
            let metadata = file.metadata()?;
            if (if directory { !metadata.is_dir() } else { !metadata.is_file() })
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
                || files::file_identity(&file)? != expected
            {
                return Err(super::super::invalid(
                    "Staged object identity changed before publication",
                ));
            }
        }
        security::verify(&self.handles[index], &self.sid, &mut phase).map_err(io::Error::other)?;
        phase.context(context).map_err(io::Error::other)?;
        native_rename::rename(
            &self.handles[index],
            native_rename::Target::SameDirectory(destination),
            matches!(publication, Publication::File { replace: true }),
            &mut phase,
        )
        .map_err(io::Error::other)?;
        if !directory {
            self.handles[index].close().map_err(io::Error::other)?;
        }
        Ok(())
    }
}
