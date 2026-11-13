// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode a fixed three-case receipt only after actual native and worker teardown.
use super::*;
impl Fixture {
    pub(super) fn receipt(&self) -> Result<serde_json::Value> {
        if !self.finished
            || !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || !self.cases.iter().all(CaseProgress::complete)
        {
            return Err("Registry fixture receipt requires all actual retirement evidence".into());
        }
        let workers = self.workers.ok_or("Registry fixture lacks actual worker settlement")?;
        if !(workers.async_finished && workers.cpu_finished && workers.native_finished) {
            return Err("Registry fixture workers remain unresolved".into());
        }
        let cases = self.cases.each_ref().map(|case| {
            serde_json::json!({
                "expected_exit_code": case.expected,
            "registry_process_id": case.process.map(|process| process.id.get()),
                "contained_started": case.started,
                "actual_exit_code": case.exited,
                "family_settled": case.settled,
                "retired": case.retired,
                "shutdown_id": case.stop.map(|stop| stop.0.get()),
                "shutdown_budget_nanoseconds": case.stop.map(|(_, budget)| serde_json::json!({
                    "started": budget.started.as_nanos(),
                    "graceful_until": budget.graceful_until.as_nanos(),
                    "settle_until": budget.settle_until.as_nanos(),
                })),
            })
        });
        Ok(serde_json::json!({
            "schema_version": 2,
            "qualification": "same_executable_registry_exit",
            "stall_seconds": sugarsubstitute_execution::qualification_time::STALL.as_secs(), "custody_seconds": crate::campaign_watch::CUSTODY.as_secs(),
            "grace_seconds": 10,
            "settle_seconds": 15,
            "explicit_empty_environment": true,
            "cases": cases,
            "registry_empty": true,
            "execution_settled": true,
            "workers": {
                "async_finished": workers.async_finished,
                "cpu_finished": workers.cpu_finished,
                "native_finished": workers.native_finished,
            },
            "finished_after_native_teardown": true,
        }))
    }
}
