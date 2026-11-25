// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Relay the owned child's GPU phase windows as the parent's own structured events.
use sugarsubstitute_contracts::lifecycle::RunKey;
use sugarsubstitute_image_gpu::timing_diagnostic::Record;

/// Run comes from the actual authenticated output batch, never from child text; the phase is one
/// of the producer's fixed names and every other field is a decoded number.
pub(in super::super) fn relay(run: RunKey, record: Record) {
    tracing::info!(
        operation = "desktop_child_image_gpu_timing",
        run = ?run,
        phase = record.phase,
        vendor = record.vendor,
        device = record.device,
        window_us = record.window_us,
        completed = record.completed,
        total_us = record.total_us,
        max_us = record.max_us,
        bytes = record.bytes,
        work_items = record.work_items,
        wait_timeouts = record.wait_timeouts,
        active = record.active,
        peak_active = record.peak_active,
        busy_us = record.busy_us,
        "Observed owned child's GPU phase window"
    );
}
