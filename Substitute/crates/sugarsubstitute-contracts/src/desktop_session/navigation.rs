// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate independent scene, batch-grid and source navigation within one occurrence.
use super::wire::SessionEntry;

/// Preserve navigation intent independently of the final artifact's inspection camera.
pub(super) fn valid(entry: &SessionEntry) -> bool {
    let navigation = &entry.view.navigation;
    if navigation.scene_overview && navigation.scene_id.is_some() {
        return false;
    }
    if navigation
        .source_id
        .as_ref()
        .is_some_and(|id| !entry.sources.iter().any(|source| source.id == *id))
    {
        return false;
    }
    if navigation.scene_id.as_ref().is_some_and(|id| {
        !entry
            .submissions
            .iter()
            .any(|item| item.scene.as_ref().is_some_and(|scene| scene.scene_id == *id))
            && !entry
                .outputs
                .iter()
                .any(|item| item.scene.as_ref().is_some_and(|scene| scene.scene_id == *id))
    }) {
        return false;
    }
    if navigation.batch_overview {
        return true;
    }
    let outputs: Vec<_> = entry
        .outputs
        .iter()
        .filter(|item| {
            navigation.source_id.as_ref().is_none_or(|id| item.source_id == *id)
                && (navigation.scene_overview
                    || item.scene.as_ref().map(|scene| &scene.scene_id)
                        == navigation.scene_id.as_ref())
        })
        .collect();
    if outputs.is_empty() {
        navigation.batch_index == 0
    } else {
        outputs.iter().any(|item| item.batch_index == navigation.batch_index)
    }
}
