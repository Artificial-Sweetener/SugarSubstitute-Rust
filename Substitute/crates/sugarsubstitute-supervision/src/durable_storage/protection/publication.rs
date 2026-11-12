// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a completed protected filesystem object through its retained native handle.
use super::*;
use crate::durable_storage::{files, protected::native_rename};
use std::{
    fs::OpenOptions,
    os::windows::{fs::OpenOptionsExt, io::IntoRawHandle},
};

/// Select publication semantics after the caller verifies ownership of any replaced file.
pub(crate) enum Publication {
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
    pub(crate) fn publish(
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
        self.publish_retained(source, destination, expected, publication, context)
    }

    /// Publish only the stable executable from this installation's selection-state folder.
    /// The retained root prevents destination redirection; source identity and protection
    /// are rechecked on the rename handle before the same-volume activation.
    pub(crate) fn publish_entry(
        &mut self,
        source: &Path,
        root: &Directory,
        expected: (u32, u32, u32),
        replace: bool,
        context: &JobContext,
    ) -> io::Result<()> {
        let layout = sugarsubstitute_configuration::layout::InstallationLayout::new(root.path());
        if source.parent() != Some(layout.state().join("selection").as_path()) {
            return Err(super::super::invalid(
                "Entry publication requires selection-state staging",
            ));
        }
        self.publish_retained(
            source,
            &layout.entry(),
            expected,
            Publication::File { replace },
            context,
        )
    }

    /// Keep both directory chains pinned through the checked handle-based rename.
    fn publish_retained(
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
        let _parent = Directory::open(
            source.parent().ok_or_else(|| super::super::invalid("Publication parent missing"))?,
            false,
        )?;
        let _destination_parent = Directory::open(
            destination
                .parent()
                .ok_or_else(|| super::super::invalid("Publication destination parent missing"))?,
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
            if source.parent() == destination.parent() {
                native_rename::Target::SameDirectory(destination)
            } else {
                native_rename::Target::Absolute(destination)
            },
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
