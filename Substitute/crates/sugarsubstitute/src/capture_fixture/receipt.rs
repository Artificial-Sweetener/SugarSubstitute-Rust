// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode bounded post-teardown facts without foreign paths or original causes.
use super::*;
/// Preserve an OS code from a bounded cause chain without formatting private text.
pub(super) fn native_os_code(mut cause: &(dyn Error + 'static)) -> Option<i32> {
    for _ in 0..8 {
        if let Some(code) =
            cause.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error)
        {
            return Some(code);
        }
        cause = cause.source()?;
    }
    None
}
/// Encode fixed naming/bundle facts without emitting local filesystem paths.
pub(super) fn encoded_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    text
}
impl Fixture {
    /// Record partial native facts and retained charges before propagating failure.
    pub(super) fn receipt(&self, passed: bool) -> serde_json::Value {
        let snapshot = self.capture.snapshot();
        let handler = snapshot.handler.map(|identity| serde_json::json!({
            "process_id": identity.process_id, "creation_time_100ns": identity.creation_time_100ns,
        }));
        let shutdown = self.stop.as_ref().map(|stop| serde_json::json!({
            "started": stop.budget.started.as_nanos(), "graceful_until": stop.budget.graceful_until.as_nanos(),
            "settle_until": stop.budget.settle_until.as_nanos(),
        }));
        let workers = self.workers.map(|w| {
            serde_json::json!({ "async_finished": w.async_finished,
            "cpu_finished": w.cpu_finished, "native_finished": w.native_finished })
        });
        let selected = self.capture.registration_build();
        let build = match selected.as_ref() {
            Some(facts) => match facts.as_str() {
                Ok(value) => serde_json::json!({"state":"observed","build":value}),
                Err(_) => serde_json::json!({"state":"invalid_cached_identity"}),
            },
            None => serde_json::json!({"state":"unavailable"}),
        };
        serde_json::json!({
            "schema_version": 3,
            "kind": "native-capture-registration",
            "passed": passed,
            "coordinator": { "namespace": encoded_hex(self.options.coordinator.namespace.bytes()),
                "epoch": encoded_hex(self.options.coordinator.epoch.bytes()) },
            "runtime_bundle": {"selection":"authenticated-current-process-installed-release","registration_identity":build},
            "registration_budget_nanoseconds": { "started": self.budget.started().as_nanos(), "until": self.budget.until().as_nanos() },
            "shutdown_budget_nanoseconds": shutdown,
            "jobs": { "handler_watch": specification::WATCH_JOB, "registration": specification::START_JOB,
                "start_admitted": self.start_admitted, "registration_terminal_seen": self.start_terminal_seen,
                "first_close": (self.demand.admitted != 0).then_some(3_u64),
                "last_close": self.demand.last_job, "close_admissions": self.demand.admitted,
                "close_terminal_successes": self.demand.successful_terminals, "close_terminal_failures": self.demand.failed_terminals },
            "capture": { "registered_observed": self.evidence.registered, "handler": handler,
                "actual_handler_exit": snapshot.handler_exit, "exit_observed": self.evidence.exit,
                "closed_observed": self.evidence.closed, "native_retired": snapshot.state == CaptureState::Closed,
                "callback_pin_bytes": snapshot.callback_pin_bytes,
                "callback_pin_reservation_bytes": snapshot.callback_pin_reservation_bytes,
                "retained_registration_scope_bytes_after_worker_settlement": self.retained_bytes_at_finish },
            "workers": workers,
            "finish_shutdown_completed": self.finished,
            "failures": { "retained": self.failures.count(), "omitted": self.failures.omitted(),
                "capture_omitted": self.capture.omitted_error_count() },
            "capture_errors": self.capture.errors().take(9).map(|cause| serde_json::json!({
                "operation": format!("{:?}", cause.operation()), "native_os_code": native_os_code(cause),
            })).collect::<Vec<_>>(),
            "scope": { "desktop_connect": false, "fault_capture": false, "incident_reporting": false, "minidump_symbolization": false },
        })
    }
}
