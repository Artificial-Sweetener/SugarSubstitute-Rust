// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the existing pinned ancestry while retaining only the next native directory handle.
use super::*;

/// One descendant pin whose type keeps its preceding directory chain borrowed until retirement.
pub(crate) struct Child<'a> {
    directory: Directory,
    _parent: &'a Directory,
}
impl std::ops::Deref for Child<'_> {
    type Target = Directory;
    fn deref(&self) -> &Self::Target {
        &self.directory
    }
}
impl Directory {
    /// Descend one validated component while borrowing the already pinned ancestry.
    /// This prevents recursive inventory from reopening every ancestor at every depth.
    pub(crate) fn child(&self, leaf: &str) -> io::Result<Child<'_>> {
        relative(leaf)?;
        if leaf.contains('/') {
            return Err(invalid("Directory child must be one component"));
        }
        let path = self.path.join(leaf);
        lexical(&path)?;
        let pin = open_component(&path, false, &mut |path| std::fs::create_dir(path))?;
        Ok(Child { directory: Directory { path, _pins: vec![pin] }, _parent: self })
    }
}
