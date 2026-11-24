// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Relay the owned child's canvas presentation windows as the parent's own structured events.
use sugarsubstitute_contracts::lifecycle::RunKey;
use sugarsubstitute_desktop::canvas_present_diagnostic::Record;

/// Run comes from the actual authenticated output batch, never from child text; every other
/// field is a decoded number, so no child-supplied text reaches the parent's log.
pub(in super::super) fn relay(run: RunKey, record: Record) {
    tracing::info!(
        operation = "desktop_child_canvas_present",
        run = ?run,
        window_us = record.window_us,
        presents = record.presents,
        created = record.created,
        updated = record.updated,
        wrapped = record.wrapped,
        source_bytes = record.source_bytes,
        fill_us = record.fill_us,
        draw_us = record.draw_us,
        commit_us = record.commit_us,
        slowest_us = record.slowest_us,
        "Observed owned child's canvas presentation window"
    );
}
