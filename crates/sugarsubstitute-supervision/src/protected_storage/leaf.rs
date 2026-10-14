// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persistent profile leaves preserve existing data under retained protected parents.
use super::*;

impl ProtectedStorageRoot {
    pub(crate) fn profile_folder(&self) -> Result<(&Path, [u8; 24], &[u8]), ClaimError> {
        let state = self.state.as_ref().ok_or_else(|| ClaimError::invalid("retired_storage"))?;
        if !state.budget.is_ordinary() {
            return Err(state.budget.rejection("profile_storage_origin"));
        }
        if !state.profile_leaf {
            return Err(ClaimError::invalid("not_profile_storage"));
        }
        let (path, identity, _) =
            state.root.as_ref().ok_or_else(|| ClaimError::invalid("unresolved_profile_storage"))?;
        Ok((path.path()?, *identity, state.sid.bytes()?))
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
        if !self.state_mut()?.budget.is_ordinary() {
            return Err(self.state_mut()?.budget.rejection("profile_storage_origin"));
        }
        paths::validate_local_shape(path).map_err(ClaimError::invalid)?;
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
        let path = parent.path()?.join(child);
        state.profile_leaf = true;
        let index = directories::open_relative(
            &mut state.handles,
            state.sid.bytes()?,
            state.budget.clone(),
            directories::RelativeDirectory {
                path: &path,
                parent: parent_index,
                mode: directories::Mode::OpenOrCreate,
                output: None,
            },
            context,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&state.budget)?,
        )?;
        let index = index.index;
        let canonical = paths::final_path(
            &state.handles[index],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[index].budget(),
            )?,
            crate::protected_storage::native_name::NativeNameDestination::Ordinary,
        )
        .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?;
        let identity = paths::file_identity(
            &state.handles[index],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[index].budget(),
            )?,
        )?;
        check(context)?;
        state.root = Some((NativeName::Ordinary(canonical), identity, index));
        Ok(())
    }
}
