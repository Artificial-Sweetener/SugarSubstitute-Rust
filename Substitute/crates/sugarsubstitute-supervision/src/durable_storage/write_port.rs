// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The one seam every update file write passes through: its bytes and its publication.
//!
//! Production writes the real file system with no other effect. Test builds consult a fault plan
//! registered for the folder holding the destination, so headless tests can fail the Nth or a
//! named update write with a full disk, denied access, a torn write or a power cut.
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

#[cfg(test)]
pub(crate) mod fake;

/// The update file a write produces. Tests name a write by this kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UpdateFile {
    /// The selection record and the records published under its writer.
    Selection,
    /// The pointer naming the engine the stable entry starts.
    LaunchPointer,
    /// The stable entry, its record and its transition journal.
    Launcher,
    /// The trust checkpoint and the records bound to the trusted repository.
    Trust,
    /// The release history that authorises a prepared generation.
    ReleaseHistory,
    /// The local receipt for a prepared update.
    Receipt,
    /// Generation payload files and the records written beside them.
    Payload,
}

/// One update file write in progress: bytes into its stage, then its publication.
pub(crate) struct FileWrite {
    #[cfg(test)]
    fault: Option<fake::Armed>,
}

/// Begin one write of `kind` whose published name will be `destination`.
pub(crate) fn begin(kind: UpdateFile, destination: &Path) -> FileWrite {
    #[cfg(test)]
    {
        FileWrite { fault: fake::begin(kind, destination) }
    }
    #[cfg(not(test))]
    {
        let _ = (kind, destination);
        FileWrite {}
    }
}

impl FileWrite {
    /// Append `bytes` to the unpublished stage. A write may arrive in several chunks.
    pub(crate) fn bytes(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        #[cfg(test)]
        if let Some(fault) = &mut self.fault {
            return fault.bytes(file, bytes);
        }
        file.write_all(bytes)
    }

    /// Run the rename or replacement that publishes the complete, flushed stage.
    pub(crate) fn publish<T>(&mut self, rename: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        #[cfg(test)]
        if let Some(fault) = &mut self.fault {
            fault.publish()?;
        }
        rename()
    }

    /// Remove the unpublished stage of a write that failed. Nothing runs once the test fake has
    /// cut the power, so a later writer must reclaim what a power cut leaves.
    pub(crate) fn abandon(&self, remove: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        #[cfg(test)]
        if let Some(fault) = &self.fault {
            fault.abandon()?;
        }
        remove()
    }
}
