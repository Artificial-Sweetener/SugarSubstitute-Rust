// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emit bounded cohort evidence after actual native loop and worker teardown.
use super::*;

impl Fixture {
    pub(super) fn receipt(&self) -> Result<serde_json::Value> {
        if !self.finished
            || !self.registry.is_empty()
            || !self.sentinel.complete()
            || !self
                .cases
                .iter()
                .all(|case| case.started && case.evidence.complete() && case.session.is_none())
        {
            return Err("Held receipt lacks actual complete cohort evidence".into());
        }
        let workers = self.workers.ok_or("Held receipt lacks worker settlement")?;
        if !(workers.async_finished && workers.cpu_finished && workers.native_finished) {
            return Err("Held receipt workers remain live".into());
        }
        let cases = self.cases.iter().enumerate().map(|(index, case)| {
            let evidence = &case.evidence;
            serde_json::json!({ "registry_process_id": evidence.process.map(|key| key.id.get()),
                "correlation": evidence.correlation.get(), "expected_exit_code": evidence.expected,
                "contained_started": case.started, "prepared": evidence.prepared, "ready": evidence.ready,
                "root_watch_job_id": evidence.root_job.map(JobId::get),
                "actual_cancelled_watch_job_id": evidence.retirement.map(JobId::get),
                "expected_non_exit_failure": evidence.expected_failure,
                "responded_after_actual_watch_cancellation": evidence.responded,
                "explicit_release_sent": evidence.session_report.is_some_and(|report| report.release_sent),
                "actual_exit_code": evidence.exited, "shutdown_id": evidence.shutdown.map(ShutdownId::get),
                "shutdown_budget_nanoseconds": self.cleanup[index].stop.as_ref().map(|stop| serde_json::json!({
                    "started":stop.budget.started.as_nanos(), "graceful_until":stop.budget.graceful_until.as_nanos(),
                    "settle_until":stop.budget.settle_until.as_nanos() })),
                "family_settled": evidence.settled, "retired": evidence.retired })
        }).collect::<Vec<_>>();
        Ok(serde_json::json!({ "schema_version":1, "qualification":"same_executable_held64_cancel",
            "held_count":HELD, "inner_deadline_seconds":60, "grace_seconds":10, "settle_seconds":15,
            "unchanged_child_deadline_qpc":self.child_deadline.ticks(),
            "qpc_frequency":self.child_deadline.frequency(), "explicit_empty_environment":true,
            "explicit_fixture_handle_count":4, "cases":cases,
            "sentinel": { "registry_process_id":self.sentinel.process.map(|key| key.id.get()),
                "root_watch_job_id":self.sentinel_watch.map(JobId::get),
                "contained_started":self.sentinel.started, "actual_exit_code":self.sentinel.exited,
                "family_settled":self.sentinel.settled, "retired":self.sentinel.retired },
            "registry_empty":true, "execution_settled":true,
            "workers": { "async_finished":workers.async_finished, "cpu_finished":workers.cpu_finished,
                "native_finished":workers.native_finished }, "finished_after_native_teardown":true }))
    }
}
