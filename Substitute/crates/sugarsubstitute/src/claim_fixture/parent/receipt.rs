// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe exact diagnostic evidence after all actual owners finish.
use super::*;
impl Fixture {
    pub(super) fn receipt(&self) -> Result<serde_json::Value> {
        let workers = self.workers.ok_or("Claim campaign lacks worker retirement")?;
        let key = self.key.ok_or("Claim campaign lacks native coordinator")?;
        let managed = self.managed.as_ref().ok_or("Claim campaign lacks managed family")?;
        let stop = managed.stop.as_ref().ok_or("Managed family lacks original StopPlan")?.budget;
        let shutdown = managed.progress.stop.ok_or("Managed family lacks shutdown identity")?.0;
        let reserved = self.scope.as_ref().map_or(0, |s| s.snapshot().reserved_bytes)
            + self.cleanup_scope.as_ref().map_or(0, |s| s.snapshot().reserved_bytes)
            + self.native_scope.as_ref().map_or(0, |s| s.snapshot().reserved_bytes);
        if !self.finished
            || !self.claims_closed
            || self.resource.is_some()
            || self.resolution.is_some()
            || self.results.len() != 7
            || self.child.is_some()
            || self.registry.as_ref().is_none_or(|r| !r.is_empty())
            || self.managed.as_ref().is_none_or(|f| !f.progress.complete())
            || !self.premature_release_rejected
            || !self.managed_released
            || !self.root_exit_before_release
            || !workers.async_finished
            || !workers.cpu_finished
            || !workers.native_finished
            || reserved != 0
        {
            return Err(
                "Claim campaign lacks complete observed teardown/containment evidence".into()
            );
        }
        Ok(serde_json::json!({
            "schema_version":2,"qualification":"windows_resource_claims",
            "namespace":hex_text(key.namespace.bytes()),"epoch":hex_text(key.epoch.bytes()),
            "stall_seconds":sugarsubstitute_execution::qualification_time::STALL.as_secs(),"custody_seconds":crate::campaign_watch::CUSTODY.as_secs(),"original_family_grace_seconds":10,"original_family_settlement_seconds":15,
            "original_execution_deadline_nanoseconds":self.execution_deadline.as_nanos(),
            "original_child_deadline_qpc":self.child_deadline.ticks(),"qpc_frequency":self.child_deadline.frequency(),
            "managed_root_exit_observed_before_release":self.root_exit_before_release,"premature_backend_release_rejected_busy":self.premature_release_rejected,
            "backend_release_observed_before_registry_retirement":self.managed_released,
            "managed_family":{
                "process_id":managed.ticket.process().id.get(),"creation_job":managed.creation.get(),"root_watch_job":managed.root_watch.get(),
                "shutdown_id":shutdown.get(),"shutdown_budget_nanoseconds":{"started":stop.started.as_nanos(),"graceful_until":stop.graceful_until.as_nanos(),"settle_until":stop.settle_until.as_nanos()},
                "contained_started":managed.progress.started,"actual_root_exit":managed.progress.exited,
                "family_settled":managed.progress.settled,"registry_retired":managed.progress.retired,
            },
            "active_predecessor_rejected_across_two_probes":self.results.iter().filter(|r|r["role"]=="active-predecessor" && r["diagnostic_child_receipt"]["actual_failure_operation"]=="unresolved_claim_predecessor").count()==2,
            "children":self.results,"claims_closed":self.claims_closed,"registry_empty":true,
            "async_finished":workers.async_finished,"cpu_finished":workers.cpu_finished,"native_finished":workers.native_finished,
            "reserved_bytes_after_teardown":reserved,"native_teardown_finished":self.finished,
        }))
    }
}
