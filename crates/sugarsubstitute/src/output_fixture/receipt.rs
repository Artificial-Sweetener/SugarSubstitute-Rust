// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emit fixed schema-one four-case evidence only after actual complete teardown.
use super::{Result, evidence::Case, parent::Parent};
fn terminal(kind: Option<sugarsubstitute_contracts::execution::TerminalKind>) -> &'static str {
    use sugarsubstitute_contracts::execution::TerminalKind;
    match kind {
        Some(TerminalKind::Success) => "success",
        Some(TerminalKind::Cancelled) => "cancelled",
        _ => "unaccepted",
    }
}
impl Parent {
    pub(super) fn receipt(&self) -> Result<serde_json::Value> {
        if !self.finished
            || !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || !self.cases.iter().all(Case::complete)
        {
            return Err("Output receipt requires every actual independent milestone".into());
        }
        let workers = self.workers.ok_or("Output receipt lacks actual worker settlement")?;
        if !(workers.async_finished && workers.cpu_finished && workers.native_finished) {
            return Err("Output workers are not actually finished".into());
        }
        let mut cases = Vec::with_capacity(4);
        for case in &self.cases {
            let report = case.output_report.ok_or("Output final native report missing")?;
            let diagnostics = case.diagnostics.ok_or("Output transcript evidence missing")?;
            let streams = [0,1].map(|i| serde_json::json!({"stream":if i == 0 {"stdout"} else {"stderr"}, "read_bytes":report.counters[i].read_bytes,"dropped_bytes":report.counters[i].dropped_bytes,"dropped_batches":report.counters[i].dropped_batches,"delivered_bytes":case.delivered[i],"gap_bytes":case.gaps[i],"last_sequence":case.sequences[i],"eof":report.eof[i],"pending_read":report.pending_reads[i],"pending_read_issues":report.pending_read_issues[i]}));
            cases.push(serde_json::json!({"case":case.role.name(),"resource_id":case.resource.id.get(),"resource_generation":case.resource.generation.get(),"run_instance":case.run.instance.get(),"run_generation":case.run.generation.get(),"registry_process_id":case.process.map(|key| key.id.get()),"handle_list_count":if case.role.authenticated() {4} else {3},"explicit_empty_environment":true,"contained_started":case.started,"actual_exit_code":case.exited,"output_terminal":terminal(case.output_terminal),"native_output_retired":report.retired,"streams":streams,"held_loan_busy":case.guard_busy,"actual_peer_authenticated":case.channel.as_ref().is_some_and(|channel|channel.verified),"actual_pipe_terminal":case.channel.as_ref().is_none_or(|channel|channel.terminal),"expected_pipe_disconnect_code":case.channel.as_ref().and_then(|channel|channel.expected_disconnect.as_ref()).and_then(|failure|failure.win32_code()),"original_diagnostic_write_win32_code":case.diagnostic_error,"retained_pending_evidence_before_cancel":case.pending_before_cancel.map(|r|r.pending_reads),"actual_pending_issue_counts_before_cancel":case.pending_before_cancel.map(|r|r.pending_read_issues),"root_exit_before_eof":case.root_before_eof,"complete_two_member_inventory":case.complete_inventory,"owned_force_succeeded":case.forced,"exact_descendant_exit_verified":case.member_verified,"descendant":case.channel.as_ref().and_then(|channel|channel.descendant).map(|(pid,birth)|serde_json::json!({"native_pid":pid,"creation_time_100ns":birth,"termination_class":"owned_job_force","graceful_child_worker_join":false})),"family_settled":case.settled,"family_retired":case.retired,"transcript":{"unicode_replacements":diagnostics.unicode_replacements,"malformed_escapes":diagnostics.malformed_escapes,"dropped_bytes":diagnostics.dropped_bytes,"skipped_batches":diagnostics.skipped_batches,"truncated_bytes":diagnostics.truncated_bytes},"shutdown":case.stop.as_ref().map(|stop|serde_json::json!({"id":stop.id.get(),"started_nanoseconds":stop.budget.started.as_nanos(),"graceful_until_nanoseconds":stop.budget.graceful_until.as_nanos(),"settle_until_nanoseconds":stop.budget.settle_until.as_nanos()}))}));
        }
        Ok(
            serde_json::json!({"schema_version":1,"qualification":"same_executable_contained_output","original_campaign_seconds":60,"stdio_mode_excludes_fixture_events":true,"cases":cases,"registry_empty":true,"finished_after_native_teardown":true,"workers":{"async_finished":workers.async_finished,"cpu_finished":workers.cpu_finished,"native_finished":workers.native_finished}}),
        )
    }
}
