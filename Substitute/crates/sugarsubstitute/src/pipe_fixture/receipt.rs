// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish bounded actual milestones after native teardown without capability material.
use super::{Result, evidence::Case, parent::Parent};

impl Parent {
    pub(super) fn receipt(&self) -> Result<serde_json::Value> {
        if !self.finished
            || !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || !self.cases.iter().all(Case::complete)
        {
            return Err("Lifecycle receipt requires all actual independent milestones".into());
        }
        let workers = self.workers.ok_or("Lifecycle receipt lacks actual worker settlement")?;
        if !(workers.async_finished && workers.cpu_finished && workers.native_finished) {
            return Err("Lifecycle receipt has unfinished workers".into());
        }
        let cases = self.cases.each_ref().map(|case| serde_json::json!({
            "case": case.kind.name(), "registry_process_id": case.process.map(|key| key.id.get()),
            "run_instance": case.run.instance.get(), "run_generation": case.run.generation.get(),
            "contained_started": case.started, "bootstrap_prepared": case.prepared, "pipe_connected": case.connected,
            "hello_decoded": case.hello, "actual_peer_authenticated": case.peer.is_some(),
            "verified_peer": case.peer.map(|peer| serde_json::json!({"native_pid": peer.process_id(), "creation_time_100ns": peer.creation_time_100ns(), "session_id": peer.session_id()})),
            "denied": case.denied, "registration_echo": case.register && case.reply_written,
            "expected_native_failure_stage": case.expected_failure.as_ref().map(|error| error.native_cause().stage()),
            "pipe_actual_terminal_consumed": case.pipe_terminal,
            "actual_exit_code": case.exited, "family_settled": case.settled, "family_retired": case.retired,
            "shutdown": case.stop.as_ref().map(|stop| serde_json::json!({"id": stop.id.get(), "started_nanoseconds": stop.budget.started.as_nanos(), "graceful_until_nanoseconds": stop.budget.graceful_until.as_nanos(), "settle_until_nanoseconds": stop.budget.settle_until.as_nanos()})),
        }));
        Ok(
            serde_json::json!({"schema_version": 1, "qualification": "same_executable_authenticated_lifecycle_transport",
            "inner_deadline_seconds": 60, "explicit_empty_environment": true, "readonly_section_only_inheritance": true,
            "cases": cases, "registry_empty": true, "finished_after_native_teardown": true,
            "workers": {"async_finished": workers.async_finished, "cpu_finished": workers.cpu_finished, "native_finished": workers.native_finished}}),
        )
    }
}
