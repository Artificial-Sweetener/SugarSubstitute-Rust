// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate coherent scene groups across accepted pending prompts and final receipts.
use super::wire::SessionEntry;
use std::collections::BTreeMap;

/// Preserve one scene identity per group/order and immutable facts for each scene identity.
pub(super) fn valid(entry: &SessionEntry) -> bool {
    let mut slots = BTreeMap::new();
    let mut facts = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for scene in entry
        .submissions
        .iter()
        .filter_map(|item| item.scene.as_ref())
        .chain(entry.outputs.iter().filter_map(|item| item.scene.as_ref()))
    {
        let slot = (&scene.group_id, scene.order);
        let identity = (&scene.group_id, &scene.scene_id);
        let description = (scene.order, scene.count, &scene.title);
        if slots.insert(slot, &scene.scene_id).is_some_and(|old| old != &scene.scene_id)
            || facts.insert(identity, description).is_some_and(|old| old != description)
            || counts.insert(&scene.group_id, scene.count).is_some_and(|old| old != scene.count)
        {
            return false;
        }
    }
    true
}
