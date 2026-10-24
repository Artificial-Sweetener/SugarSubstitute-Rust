// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the installer's isolated browser directory and verify its actual native file identity.
use super::{check, files::Directory, invalid};
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::fs::OpenOptionsExt,
    path::Path,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
/// An OS-backed exclusion and pinned directory chain retained until browser teardown.
pub struct InstallerProfile {
    _directory: Directory,
    _exclusion: File,
    _memory: MemoryClaim,
}
impl InstallerProfile {
    /// Verify the WebView2-reported directory against the requested directory by file ID.
    /// Run off the UI thread. The resulting lease must outlive the native browser.
    /// # Errors
    /// Rejects redirection, links, concurrent use, cancellation and native query failures.
    pub fn acquire(expected: &Path, observed: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let directory = Directory::open(expected, true)?;
        let observed = Directory::open(observed, false)?;
        if directory.identity()? != observed.identity()? {
            return Err(invalid("Installer browser profile was redirected"));
        }
        let exclusion = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(1)
            .open(directory.path.join(".setup-profile-lock"))?;
        exclusion.try_lock().map_err(io::Error::other)?;
        check(context)?;
        Ok(Self { _directory: directory, _exclusion: exclusion, _memory: memory })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_directory_identity_distinguishes_equal_named_private_profiles() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let first = Directory::open(&root.path().join("a/profile"), true)?;
        let second = Directory::open(&root.path().join("b/profile"), true)?;
        let repeated = Directory::open(&first.path, false)?;
        assert_eq!(first.identity()?, repeated.identity()?);
        assert_ne!(first.identity()?, second.identity()?);
        Ok(())
    }
}
