// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain accepted prompt ownership independently of output arrival and live node numbers.
use super::{
    primitives::uuid,
    wire::{SessionEntry, SessionScope},
};
use std::collections::BTreeSet;

/// Match final receipts to an exact accepted prompt and its frozen source/scene identity.
pub(super) fn valid(entry: &SessionEntry, scope: &SessionScope) -> bool {
    let mut prompts = BTreeSet::new();
    let mut runs = BTreeSet::new();
    for submission in &entry.submissions {
        if submission.entry_id != entry.id
            || submission.backend != scope.backend
            || !uuid(&submission.run_id)
            || submission.prompt_id.is_empty()
            || !prompts.insert(&submission.prompt_id)
            || !runs.insert((
                &submission.run_id,
                submission.scene.as_ref().map(|scene| &scene.scene_id),
            ))
            || submission.graph_revision > entry.document.graph_revision
            || submission.scene.as_ref().is_some_and(|scene| {
                !uuid(&scene.group_id) || !uuid(&scene.scene_id) || scene.order >= scene.count
            })
        {
            return false;
        }
        let mut sources = BTreeSet::new();
        for source in &submission.sources {
            if !uuid(&source.id)
                || !sources.insert(&source.id)
                || source.node_id.is_empty()
                || source.execution_path.is_empty()
                || source.graph_revision != submission.graph_revision
            {
                return false;
            }
        }
    }
    entry.outputs.iter().all(|output| {
        output.prompt_id.as_ref().is_none_or(|prompt| {
            entry.submissions.iter().any(|submission| {
                submission.prompt_id == *prompt
                    && submission.run_id == output.run_id
                    && submission.run_order == output.run_order
                    && submission.scene == output.scene
                    && submission.sources.iter().any(|source| source.id == output.source_id)
            })
        })
    })
}
