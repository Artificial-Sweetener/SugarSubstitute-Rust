// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin the folder that holds a selected one, which alone may be a bare drive root.
use super::*;

impl Directory {
    /// Pin the existing parent of `child`, a folder or file named directly inside it.
    ///
    /// A folder directly inside a drive root, such as `G:\ComfyUI`, is an ordinary selection, so
    /// its parent is the root itself. The root is only ever pinned as a parent here: no caller
    /// selects, creates or publishes a bare drive root.
    /// # Errors
    /// Rejects an invalid `child` spelling and preserves a missing or unreadable parent.
    pub(crate) fn parent_of(child: &Path) -> io::Result<Self> {
        lexical(child)?;
        let parent: PathBuf =
            child.parent().ok_or_else(|| invalid("Folder parent absent"))?.components().collect();
        Self::reopen(&parent)
    }

    /// Pin an existing location this owner already holds: a folder below a drive root, or the
    /// drive root that holds one.
    pub(super) fn reopen(path: &Path) -> io::Result<Self> {
        if !drive_root(path) {
            lexical(path)?;
        }
        // Nothing is created: an absent component is reported as it was observed.
        Self::pin(path, false, &mut |_| Err(invalid("An existing folder is required")))
    }
}

/// Recognize exactly a local drive's root, in DOS or verbatim spelling.
fn drive_root(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        && matches!(components.next(), Some(Component::RootDir))
        && components.next().is_none()
}
