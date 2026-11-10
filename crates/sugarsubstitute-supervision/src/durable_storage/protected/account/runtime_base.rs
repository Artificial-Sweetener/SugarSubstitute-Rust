// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin the installation-local base that holds per-account runtime records.
//!
//! Runtime records live below the installation they describe, never in account profile
//! folders. The base is the installation root named by the configuration layout owner.
use super::*;
mod historical;
pub(crate) use historical::resolve as historical;
/// Open, pin and verify an existing runtime base, preserving native output and original causes.
/// The returned final native path names every following namespace child.
pub(crate) fn pin(
    base: &Path,
    handles: &mut Vec<Handle>,
    budget: Budget,
) -> Result<PathBuf, ClaimError> {
    let base_handle = paths::open_retained(
        base,
        true,
        false,
        budget.clone(),
        handles,
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    let directory = paths::final_path(
        &handles[base_handle],
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &handles[base_handle].budget(),
        )?,
        crate::durable_storage::protected::native_name::NativeNameDestination::Ordinary,
    )
    .and_then(crate::durable_storage::protected::native_name::NativeName::into_ordinary)?;
    let base_identity = paths::file_identity(
        &handles[base_handle],
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &handles[base_handle].budget(),
        )?,
    )?;
    paths::pin_ancestors(
        &directory,
        handles,
        budget.clone(),
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    paths::verify_pinned(
        &directory,
        base_identity,
        handles,
        true,
        budget.clone(),
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    Ok(directory)
}
