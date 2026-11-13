// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Relay the owned child's closed bulk channel refusals as the parent's own structured events.
use sugarsubstitute_contracts::lifecycle::RunKey;
use sugarsubstitute_desktop::bulk_channel_diagnostic::Record;

/// Run comes from the actual authenticated output batch, never from child text; every other
/// field is a closed decoded value, so no child-supplied text reaches the parent's log.
pub(in super::super) fn relay(run: RunKey, record: Record) {
    tracing::info!(
        operation = "desktop_child_bulk_channel",
        run = ?run,
        reason = %record.miss,
        context = i64::from(record.context),
        method = %record.method,
        origin = %record.origin,
        "Observed owned child's refused bulk channel request"
    );
}

#[cfg(test)]
#[path = "bulk_tests.rs"]
mod tests;
