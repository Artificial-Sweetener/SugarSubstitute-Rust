// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exclusive generation activation uses the retained validated parent chain.
use super::*;
fn name(coordinator: CoordinatorKey) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(107);
    result.push_str("capture-v1-");
    for byte in coordinator.namespace.bytes().iter().chain(coordinator.epoch.bytes()) {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
pub(super) fn create(
    state: &mut State,
    coordinator: CoordinatorKey,
    context: &JobContext,
) -> Result<(), ClaimError> {
    if !state.budget.is_ordinary() {
        return Err(state.budget.rejection("capture_generation_origin"));
    }
    bind_context(state, context, false)?;
    check(context)?;
    if state.attempted || state.profile_leaf {
        return Err(ClaimError::invalid("generation_already_attempted"));
    }
    let (root, identity, root_index) =
        state.root.as_ref().ok_or_else(|| ClaimError::invalid("missing_protected_root"))?;
    let root_index = *root_index;
    let directory = root.path()?.join(name(coordinator));
    let database = directory.join("database");
    let metrics = directory.join("metrics");
    for path in [&directory, &database, &metrics] {
        paths::wide(path)?;
    }
    paths::verify_pinned(
        root.path()?,
        *identity,
        &mut state.handles,
        true,
        state.budget.clone(),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &(state.budget.clone()),
        )?,
    )?;
    // Retain all intended generation paths before the first create-new effect.
    state.attempted = true;
    state.generation = Some(ProtectedStorageGeneration {
        directory: directory.clone(),
        database: database.clone(),
        metrics: metrics.clone(),
        ready: false,
        coordinator,
        slots: [usize::MAX; 2],
        identities: [[0; 24]; 2],
    });
    let parent = directories::open_relative(
        &mut state.handles,
        state.sid.bytes()?,
        state.budget.clone(),
        directories::RelativeDirectory {
            path: &directory,
            parent: root_index,
            mode: directories::Mode::CreateNew,
            output: None,
        },
        context,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&state.budget)?,
    )?;
    let database_opened = directories::open_relative(
        &mut state.handles,
        state.sid.bytes()?,
        state.budget.clone(),
        directories::RelativeDirectory {
            path: &database,
            parent: parent.index,
            mode: directories::Mode::CreateNew,
            output: None,
        },
        context,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&state.budget)?,
    )?;
    let metrics_opened = directories::open_relative(
        &mut state.handles,
        state.sid.bytes()?,
        state.budget.clone(),
        directories::RelativeDirectory {
            path: &metrics,
            parent: parent.index,
            mode: directories::Mode::CreateNew,
            output: None,
        },
        context,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&state.budget)?,
    )?;
    let generation = state
        .generation
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("missing_capture_generation"))?;
    generation.slots = [database_opened.index, metrics_opened.index];
    generation.identities = [database_opened.identity, metrics_opened.identity];
    generation.ready = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_name_binds_exact_namespace_epoch_and_has_no_path_components()
    -> Result<(), Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};
        let key = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        assert_eq!(name(key), format!("capture-v1-{}{}", "01".repeat(32), "02".repeat(16)));
        assert_eq!(Path::new(&name(key)).components().count(), 1);
        let other = CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..key };
        assert_ne!(name(key), name(other));
        Ok(())
    }
}
