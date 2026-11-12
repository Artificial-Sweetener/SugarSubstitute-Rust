// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the file effects on a ComfyUI node-pack folder and on its update transaction.
//!
//! A pack folder is shared with ComfyUI, ComfyUI Manager and the user, so it is never pinned
//! for the lifetime of an update; each effect instead refuses to traverse or replace a link
//! or junction at the moment it runs. Writes go to a unique sibling that reaches the disk
//! before it is renamed over the destination, so a power cut leaves the old or the new
//! bytes, never a torn file. The install format decides what to write; this owner only
//! performs it.

use super::invalid;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const COPY_CHUNK_BYTES: usize = 1024 * 1024;

/// What one name in a pack tree currently is, observed without following links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeKind {
    /// Nothing exists at the name.
    Missing,
    /// A regular file.
    File,
    /// A plain directory.
    Directory,
    /// A symbolic link, junction or other reparse point, or a special file.
    Link,
}

/// A folder whose named children this owner reads and replaces.
#[derive(Clone, Debug)]
pub(crate) struct PackTree {
    root: PathBuf,
}

impl PackTree {
    /// Address one absolute folder; nothing is opened yet.
    pub(crate) fn at(root: &Path) -> io::Result<Self> {
        super::files::lexical(root)?;
        Ok(Self { root: root.to_owned() })
    }

    /// Address a child folder of this tree.
    pub(crate) fn child(&self, relative: &str) -> io::Result<Self> {
        Ok(Self { root: self.path(relative)? })
    }

    fn path(&self, relative: &str) -> io::Result<PathBuf> {
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        let mut path = self.root.clone();
        for segment in relative.trim_end_matches('/').split('/') {
            if matches!(segment, "" | "." | "..") || segment.contains(['\\', ':']) {
                return Err(invalid("Pack entries must be plain relative names"));
            }
            path.push(segment);
        }
        Ok(path)
    }

    /// Observe the folder itself.
    pub(crate) fn root_kind(&self) -> io::Result<NodeKind> {
        observe(&self.root)
    }

    /// Observe one entry, reporting a link when any component on its way is one.
    pub(crate) fn kind(&self, relative: &str) -> io::Result<NodeKind> {
        self.path(relative)?;
        let mut current = self.root.clone();
        let mut kind = NodeKind::Directory;
        for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
            current.push(segment);
            kind = observe(&current)?;
            if matches!(kind, NodeKind::Link | NodeKind::Missing) {
                return Ok(kind);
            }
        }
        Ok(kind)
    }

    /// Read a regular file of at most `maximum` bytes.
    pub(crate) fn read(&self, relative: &str, maximum: usize) -> io::Result<Vec<u8>> {
        let mut file = self.open_regular(relative)?;
        let limit = u64::try_from(maximum).map_err(io::Error::other)?.saturating_add(1);
        let mut bytes = Vec::new();
        (&mut file).take(limit).read_to_end(&mut bytes)?;
        if bytes.len() > maximum {
            return Err(invalid("Pack file exceeds its read bound"));
        }
        Ok(bytes)
    }

    /// Return the lowercase SHA-256 of a regular file.
    pub(crate) fn sha256(&self, relative: &str) -> io::Result<String> {
        let mut file = self.open_regular(relative)?;
        let mut digest = Sha256::new();
        let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        Ok(format!("{:x}", digest.finalize()))
    }

    fn open_regular(&self, relative: &str) -> io::Result<File> {
        if self.kind(relative)? != NodeKind::File {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Pack entry is not a regular file",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(self.path(relative)?)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(invalid("Pack entry is not a regular file"));
        }
        Ok(file)
    }

    /// Durably replace one file with `bytes`.
    pub(crate) fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        self.replace_with(relative, |output| output.write_all(bytes))
    }

    /// Durably copy one regular file of `source` over `relative` in this tree.
    pub(crate) fn copy_from(
        &self,
        relative: &str,
        source: &Self,
        source_relative: &str,
    ) -> io::Result<()> {
        let mut input = source.open_regular(source_relative)?;
        self.replace_with(relative, |output| io::copy(&mut input, output).map(|_| ()))
    }

    fn replace_with(
        &self,
        relative: &str,
        fill: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> io::Result<()> {
        let destination = self.path(relative)?;
        if matches!(self.kind(relative)?, NodeKind::Link | NodeKind::Directory) {
            return Err(invalid("A pack file cannot replace a link or folder"));
        }
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Pack entry has no name"))?;
        let temporary =
            destination.with_file_name(format!(".{name}.{}.partial", super::staging::identity()?));
        let written = (|| {
            let mut output = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
            fill(&mut output)?;
            output.sync_all()?;
            drop(output);
            if self.kind(relative)? == NodeKind::File {
                clear_read_only(&destination)?;
            }
            std::fs::rename(&temporary, &destination)
        })();
        if written.is_err() {
            match std::fs::remove_file(&temporary) {
                Ok(()) => {}
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => {
                    tracing::warn!(
                        target: "sugarsubstitute::pack_install",
                        operation = "discard_partial",
                        error = %cause,
                        "A partial pack file could not be removed"
                    );
                }
            }
        }
        written
    }

    /// Remove one file; an absent file is already removed. A link is removed, never followed.
    pub(crate) fn remove_file(&self, relative: &str) -> io::Result<()> {
        let path = self.path(relative)?;
        match self.kind(relative)? {
            NodeKind::Missing => Ok(()),
            NodeKind::Directory => Err(invalid("A pack file entry is a folder")),
            NodeKind::File | NodeKind::Link => {
                clear_read_only(&path)?;
                match std::fs::remove_file(&path) {
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(()),
                    other => other,
                }
            }
        }
    }

    /// Create a directory and its missing parents, refusing to pass through a link.
    pub(crate) fn create_directories(&self, relative: &str) -> io::Result<()> {
        let mut prefix = String::new();
        for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            match self.kind(&prefix)? {
                NodeKind::Directory => {}
                NodeKind::Missing => match std::fs::create_dir(self.path(&prefix)?) {
                    Err(cause) if cause.kind() != io::ErrorKind::AlreadyExists => {
                        return Err(cause);
                    }
                    _ if self.kind(&prefix)? != NodeKind::Directory => {
                        return Err(invalid("A pack folder was replaced while creating it"));
                    }
                    _ => {}
                },
                NodeKind::File | NodeKind::Link => {
                    return Err(invalid("A pack folder entry is a file or link"));
                }
            }
        }
        Ok(())
    }

    /// Create the tree's own folder and its parents.
    pub(crate) fn create(&self) -> io::Result<()> {
        if self.root_kind()? == NodeKind::Link {
            return Err(invalid("A pack folder is a link"));
        }
        std::fs::create_dir_all(&self.root)
    }

    /// Remove a directory when it is empty; returns whether it was removed.
    pub(crate) fn remove_empty_directory(&self, relative: &str) -> io::Result<bool> {
        if self.kind(relative)? != NodeKind::Directory {
            return Ok(false);
        }
        let path = self.path(relative)?;
        if std::fs::read_dir(&path)?.next().is_some() {
            return Ok(false);
        }
        std::fs::remove_dir(path).map(|()| true)
    }

    /// Remove this tree, which its owner created; links inside are removed, never followed.
    pub(crate) fn remove_all(&self) -> io::Result<()> {
        remove_tree(&self.root)
    }
}

fn observe(path: &Path) -> io::Result<NodeKind> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(NodeKind::Missing),
        Err(cause) => return Err(cause),
    };
    Ok(if metadata.file_type().is_symlink() || metadata.file_attributes() & REPARSE_POINT != 0 {
        NodeKind::Link
    } else if metadata.is_dir() {
        NodeKind::Directory
    } else if metadata.is_file() {
        NodeKind::File
    } else {
        NodeKind::Link
    })
}

fn clear_read_only(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        // Windows refuses to replace or delete a read-only file; the pack owns these bytes.
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn remove_tree(path: &Path) -> io::Result<()> {
    match observe(path)? {
        NodeKind::Missing => Ok(()),
        NodeKind::File => {
            clear_read_only(path)?;
            std::fs::remove_file(path)
        }
        NodeKind::Link => {
            // A directory junction is removed as a directory, a file link as a file.
            if std::fs::symlink_metadata(path)?.is_dir() {
                std::fs::remove_dir(path)
            } else {
                clear_read_only(path)?;
                std::fs::remove_file(path)
            }
        }
        NodeKind::Directory => {
            for child in std::fs::read_dir(path)? {
                remove_tree(&child?.path())?;
            }
            std::fs::remove_dir(path)
        }
    }
}
