// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persistent profile leaves preserve existing data under retained protected parents.
use super::*;

impl ProtectedStorageRoot {
    pub(crate) fn profile_folder(&self) -> Result<(&Path, [u8; 24], &[u8]), ClaimError> {
        let state = self.state.as_ref().ok_or_else(|| ClaimError::invalid("retired_storage"))?;
        if !state.profile_leaf {
            return Err(ClaimError::invalid("not_profile_storage"));
        }
        let (path, identity, _) =
            state.root.as_ref().ok_or_else(|| ClaimError::invalid("unresolved_profile_storage"))?;
        Ok((path, *identity, &state.sid))
    }
    /// Open or create only the final profile directory under an existing protected parent.
    ///
    /// Existing directories must match the account/System security policy and
    /// canonical non-reparse chain. The method performs no enumeration, content
    /// replacement or deletion. Unknown/missing ancestors remain untouched.
    ///
    /// # Errors
    /// Rejects malformed leaf names, repeated root use, foreign/security/identity
    /// uncertainty and original deadline/cancellation. Partial native handles
    /// remain with this proof for checked worker retirement.
    pub fn resolve_or_create_profile_leaf(
        &mut self,
        path: &Path,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        paths::wide(path)?;
        let child = path.file_name().ok_or_else(|| ClaimError::invalid("profile_leaf_name"))?;
        let parent = path.parent().ok_or_else(|| ClaimError::invalid("profile_parent"))?;
        if child.as_encoded_bytes().iter().any(|byte| matches!(*byte, 0 | b':' | b'/' | b'\\')) {
            return Err(ClaimError::invalid("profile_leaf_name"));
        }
        self.resolve(parent, context)?;
        let state = self.state.as_mut().ok_or_else(|| ClaimError::invalid("retired_storage"))?;
        bind_context(state, context, false)?;
        check(context)?;
        let (parent, _, parent_index) =
            state.root.take().ok_or_else(|| ClaimError::invalid("profile_parent_owner"))?;
        let path = parent.join(child);
        state.profile_leaf = true;
        let index = directories::open_relative(
            state,
            &path,
            parent_index,
            directories::Mode::OpenOrCreate,
            context,
        )?;
        let canonical = paths::final_path(&state.handles[index])?;
        let identity = paths::file_identity(&state.handles[index])?;
        check(context)?;
        state.root = Some((canonical, identity, index));
        Ok(())
    }
}
