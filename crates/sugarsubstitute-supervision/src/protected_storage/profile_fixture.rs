// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create one test-owned protected directory through the original native owner.
use super::*;

/// Create a new leaf with the genuine account descriptor, retaining native outputs until close.
pub(crate) fn create(path: &Path, context: &JobContext) -> Result<PathBuf, ClaimError> {
    let memory = Arc::new(
        context
            .memory()
            .claim(STORAGE_BYTES)
            .map_err(|cause| ClaimError::new("profile_fixture_memory", cause))?,
    );
    let budget = Budget::Protected(memory);
    let mut handles = Vec::with_capacity(HANDLE_BOUND);
    let result = (|| {
        let mut phase = native_phase::NativePhase::ordinary(&budget)?;
        let account = account::resolve_current(
            &mut handles,
            budget.clone(),
            account::PrimaryAccountAccess::Query,
            &mut phase,
        )?;
        let parent = path.parent().ok_or_else(|| ClaimError::invalid("profile_fixture_parent"))?;
        let parent =
            paths::open_retained(parent, true, false, budget.clone(), &mut handles, &mut phase)?;
        let canonical = paths::final_path(
            &handles[parent],
            &mut phase,
            native_name::NativeNameDestination::Ordinary,
        )?
        .into_ordinary()?;
        let path = canonical
            .join(path.file_name().ok_or_else(|| ClaimError::invalid("profile_fixture_leaf"))?);
        directories::open_relative(
            &mut handles,
            account.sid.bytes()?,
            budget.clone(),
            directories::RelativeDirectory {
                path: &path,
                parent,
                mode: directories::Mode::CreateNew,
            },
            context,
            &mut phase,
        )?;
        Ok(path)
    })();
    let mut cleanup = Ok(());
    for handle in handles.iter_mut().rev() {
        cleanup = ClaimError::finish(cleanup, handle.close());
    }
    ClaimError::finish(result, cleanup)
}
