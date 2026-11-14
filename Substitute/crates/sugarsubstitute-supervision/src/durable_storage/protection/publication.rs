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
        .map_err(|cause| held_destination(destination, cause))?;
        if !directory {
            self.handles[index].close().map_err(io::Error::other)?;
        }
        Ok(())
    }
}

/// A rename refused because another handle holds its destination, or the folder receiving it,
/// without sharing the access the rename needs.
const SHARING_VIOLATION: u32 = 32;

/// A publication refused because its destination, or the folder receiving it, is in use.
///
/// Carried by an [`io::ErrorKind::ResourceBusy`] error, so an owner whose publication may wait
/// for the holder can tell this refusal from every other failure. A running copy of the
/// application is one such holder: its crash-capture and diagnostic pins keep the installation
/// folder open without write sharing for as long as it runs.
#[derive(Debug)]
struct HeldDestination {
    destination: std::path::PathBuf,
    holders: String,
    cause: crate::ClaimError,
}

impl std::fmt::Display for HeldDestination {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} could not be published, {}: {}",
            self.destination.display(),
            self.holders,
            self.cause
        )
    }
}

impl std::error::Error for HeldDestination {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Whether `cause` is a publication refused because its destination, or the folder receiving
/// it, is in use.
pub(crate) fn held(cause: &io::Error) -> bool {
    cause.get_ref().is_some_and(|inner| inner.is::<HeldDestination>())
}

/// Name who holds a destination a rename found in use. Restart Manager is asked at once which
/// live processes hold the destination file; each is logged and named in the error. When none
/// holds the file, the folder receiving it is what is held, and the error says so. Any other
/// failure passes through unchanged.
fn held_destination(destination: &Path, cause: crate::ClaimError) -> io::Error {
    if cause.win32_code() != Some(SHARING_VIOLATION) {
        return io::Error::other(cause);
    }
    let mut resources = crate::profile_conflict::resources::Resources::default();
    let holders = match resources.holders(destination) {
        Ok(holders) if holders.is_empty() => {
            "no process holds the file itself, so the folder receiving it is held open".to_owned()
        }
        Ok(holders) => {
            for holder in &holders {
                tracing::info!(operation = "protected_publication_held", destination = %destination.display(),
                    holder = %holder, "Another process holds a destination open");
            }
            format!(
                "held by {}",
                holders.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            )
        }
        Err(query) => format!("its holders could not be listed: {query}"),
    };
    if let Err(close) = resources.close() {
        tracing::warn!(operation = "protected_publication_held", error = %close,
            "The holder lookup could not be closed");
    }
    io::Error::new(
        io::ErrorKind::ResourceBusy,
        HeldDestination { destination: destination.to_owned(), holders, cause },
    )
}

#[cfg(test)]
mod tests {
    use super::held_destination;
    use std::os::windows::fs::OpenOptionsExt;

    /// A rename refused because its destination is in use is a busy refusal that names who holds
    /// the file, or says the folder is what is held when no process holds the file itself. Every
    /// other failure passes through as it was.
    #[test]
    fn a_held_destination_is_a_busy_refusal_that_names_its_holder()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("record");
        std::fs::write(&destination, b"held")?;
        let refused = || {
            crate::ClaimError::new("activate_claim_record", std::io::Error::from_raw_os_error(32))
        };
        let holder = std::fs::OpenOptions::new().read(true).share_mode(0x1).open(&destination)?;
        let held = held_destination(&destination, refused());
        assert_eq!(held.kind(), std::io::ErrorKind::ResourceBusy);
        let named = held.to_string();
        assert!(named.contains(&format!("(pid {})", std::process::id())), "{named}");
        assert!(named.contains("record"), "{named}");
        drop(holder);
        let folder = held_destination(&destination, refused());
        assert_eq!(folder.kind(), std::io::ErrorKind::ResourceBusy);
        assert!(folder.to_string().contains("the folder receiving it is held open"), "{folder}");
        let other = held_destination(
            &destination,
            crate::ClaimError::new("activate_claim_record", std::io::Error::from_raw_os_error(5)),
        );
        assert_ne!(other.kind(), std::io::ErrorKind::ResourceBusy);
        assert!(!other.to_string().contains("could not be published"), "{other}");
        Ok(())
    }
}
