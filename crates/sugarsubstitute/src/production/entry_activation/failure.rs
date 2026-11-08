// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the first actual retained activation cause without detaching its native custody.
use super::*;
impl ProductionActivation {
    /// Inspect original source, recovery, incident and run owners in their existing precedence.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(cause) = self.pending.iter().flatten().find_map(|pending| {
            pending.source.as_ref().and_then(source_preparation::Preparation::failure)
        }) {
            return Some(cause);
        }
        if let Some(cause) = self
            .recovery_pending
            .iter()
            .chain(self.failed_recoveries.iter().flatten())
            .find_map(|pending| pending.failure_source())
        {
            return Some(cause);
        }
        if let Some(cause) = self.incidents.reporting.failure() {
            return Some(cause);
        }
        if let Some(cause) = self.incidents.host.failure() {
            return Some(cause);
        }
        if let Some(cause) =
            self.records.iter().flatten().find_map(|record| record.record.seed.failure())
        {
            return Some(cause);
        }
        if let Some(cause) =
            self.records.iter().flatten().find_map(|record| record.record.collection.failure())
        {
            return Some(cause);
        }
        if let Some(operation) = self
            .failure
            .as_deref()
            .and_then(|cause| cause.downcast_ref::<backend_failure::OperationFailure>())
        {
            return Some(operation.cause());
        }
        if let Some(key) = self.failure_resource
            && let Some(retained) = &self.backend
        {
            if let Some(cause) = retained.backend.cleanup_failure(key) {
                return Some(cause);
            }
            if let Some(cause) = retained.backend.failure(key) {
                return Some(cause);
            }
        }
        self.failure_run
            .and_then(|run| {
                self.records
                    .iter()
                    .flatten()
                    .map(|retained| retained.record.as_ref())
                    .filter(|record| record.run == run)
                    .find_map(|record| {
                        record
                            .parent
                            .as_ref()
                            .and_then(|parent| parent.failure())
                            .map(|cause| cause as &(dyn Error + 'static))
                            .or_else(|| {
                                record
                                    .start
                                    .as_ref()
                                    .and_then(|start| start.output.as_ref())
                                    .and_then(|output| output.failure())
                            })
                            .or_else(|| {
                                record.start.as_ref().and_then(DesktopStart::recovery_failure)
                            })
                            .or_else(|| match &record.projected.as_ref()?.outcome {
                                JobOutcome::Failure(cause) => Some(cause as &(dyn Error + 'static)),
                                _ => None,
                            })
                    })
            })
            .or(self.failure.as_deref())
    }
}
