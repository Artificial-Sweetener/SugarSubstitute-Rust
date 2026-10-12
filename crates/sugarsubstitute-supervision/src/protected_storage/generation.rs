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
    bind_context(state, context, false)?;
    check(context)?;
    if state.attempted || state.profile_leaf {
        return Err(ClaimError::invalid("generation_already_attempted"));
    }
    let (root, identity, root_index) =
        state.root.as_ref().ok_or_else(|| ClaimError::invalid("missing_protected_root"))?;
    let root_index = *root_index;
    let directory = root.join(name(coordinator));
    let database = directory.join("database");
    let metrics = directory.join("metrics");
    for path in [&directory, &database, &metrics] {
        paths::wide(path)?;
    }
    paths::verify_pinned(
        root,
        *identity,
        &mut state.handles,
        true,
        files::Budget::Protected(state.memory.clone()),
    )?;
    // Retain all intended generation paths before the first create-new effect.
    state.attempted = true;
    state.generation = Some(ProtectedStorageGeneration {
        directory: directory.clone(),
        database: database.clone(),
        metrics: metrics.clone(),
        ready: false,
    });
    let parent = directories::open_relative(
        state,
        &directory,
        root_index,
        directories::Mode::CreateNew,
        context,
    )?;
    directories::open_relative(state, &database, parent, directories::Mode::CreateNew, context)?;
    directories::open_relative(state, &metrics, parent, directories::Mode::CreateNew, context)?;
    state
        .generation
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("missing_capture_generation"))?
        .ready = true;
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
