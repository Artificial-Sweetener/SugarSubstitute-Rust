// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry a checkpoint between its versions without losing a value either version holds.
//!
//! Version 2 is version 1 with each entry's view naming where its masonry is scrolled. A version-1
//! checkpoint becomes version 2 with no scroll; a version-2 checkpoint is written as version 1 when
//! no entry names a scroll, so a page that reads only version 1 always reads what it can.
use super::wire::{self, v2};

/// Read a version-1 checkpoint as version 2, naming no scroll.
pub(super) fn upgrade(value: wire::SessionCheckpoint) -> v2::SessionCheckpoint {
    let wire::SessionCheckpoint {
        version: _,
        id,
        revision,
        previous_id,
        captured_at,
        scope,
        producer,
        frontend,
        sugarcubes,
        entries,
        active_entry,
        route,
        placement,
        assets,
    } = value;
    v2::SessionCheckpoint {
        version: 2,
        id,
        revision,
        previous_id,
        captured_at,
        scope,
        producer,
        frontend,
        sugarcubes,
        entries: entries.into_iter().map(upgrade_entry).collect(),
        active_entry,
        route,
        placement,
        assets,
    }
}

/// The version-1 form of a checkpoint whose entries name no scroll; `None` when one does.
pub(super) fn downgrade(value: &v2::SessionCheckpoint) -> Option<wire::SessionCheckpoint> {
    let entries = value.entries.iter().map(downgrade_entry).collect::<Option<Vec<_>>>()?;
    Some(wire::SessionCheckpoint {
        version: 1,
        id: value.id.clone(),
        revision: value.revision,
        previous_id: value.previous_id.clone(),
        captured_at: value.captured_at,
        scope: value.scope.clone(),
        producer: value.producer.clone(),
        frontend: value.frontend.clone(),
        sugarcubes: value.sugarcubes.clone(),
        entries,
        active_entry: value.active_entry.clone(),
        route: value.route.clone(),
        placement: value.placement.clone(),
        assets: value.assets.clone(),
    })
}

fn upgrade_entry(value: wire::SessionEntry) -> v2::SessionEntry {
    let wire::SessionEntry {
        id,
        document,
        input,
        view,
        source_revision,
        navigation_revision,
        sources,
        outputs,
        submissions,
    } = value;
    let wire::SessionView {
        canvas_enabled,
        masonry_enabled,
        editor_width,
        route,
        follow,
        mode,
        linked,
        selection,
        comparison,
        cameras,
        navigation,
        source_preview,
        shared_inspection,
    } = view;
    v2::SessionEntry {
        id,
        document,
        input,
        view: v2::SessionView {
            canvas_enabled,
            masonry_enabled,
            editor_width,
            route,
            follow,
            mode,
            linked,
            selection,
            comparison,
            cameras,
            navigation,
            source_preview,
            shared_inspection,
            scroll: None,
        },
        source_revision,
        navigation_revision,
        sources,
        outputs,
        submissions,
    }
}

fn downgrade_entry(value: &v2::SessionEntry) -> Option<wire::SessionEntry> {
    let view = &value.view;
    if view.scroll.is_some() {
        return None;
    }
    Some(wire::SessionEntry {
        id: value.id.clone(),
        document: value.document.clone(),
        input: value.input.clone(),
        view: wire::SessionView {
            canvas_enabled: view.canvas_enabled,
            masonry_enabled: view.masonry_enabled,
            editor_width: view.editor_width,
            route: view.route.clone(),
            follow: view.follow.clone(),
            mode: view.mode.clone(),
            linked: view.linked,
            selection: view.selection.clone(),
            comparison: view.comparison.clone(),
            cameras: view.cameras.clone(),
            navigation: view.navigation.clone(),
            source_preview: view.source_preview,
            shared_inspection: view.shared_inspection.clone(),
        },
        source_revision: value.source_revision,
        navigation_revision: value.navigation_revision,
        sources: value.sources.clone(),
        outputs: value.outputs.clone(),
        submissions: value.submissions.clone(),
    })
}
