// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe the independent original export ledger without retaining native authority.
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU16, Ordering},
};
use sugarsubstitute_contracts::execution::ScopeId;
pub(crate) struct Counters {
    pub(crate) outputs: AtomicU16,
    pub(crate) high_water: AtomicU16,
}
impl Counters {
    pub(crate) fn new() -> Self {
        Self { outputs: AtomicU16::new(0), high_water: AtomicU16::new(0) }
    }
}
pub(crate) struct ResourceRecord {
    pub(crate) scope_id: ScopeId,
    pub(crate) issued: bool,
    pub(crate) observation: Weak<Counters>,
}
impl ResourceRecord {
    pub(crate) fn matches(&self, counters: &Arc<Counters>) -> bool {
        Weak::ptr_eq(&self.observation, &Arc::downgrade(counters))
    }
    pub(crate) fn snapshot(&self) -> (u16, u16) {
        self.observation.upgrade().map_or((0, 0), |c| {
            let count = c.outputs.load(Ordering::Acquire);
            (count, c.high_water.load(Ordering::Acquire).max(count))
        })
    }
}
/// Append passive original custody to the same central shutdown observation.
pub(crate) fn output_custody(state: &crate::admission::State) -> Vec<crate::NativeOutputCustody> {
    state
        .report_resources
        .iter()
        .filter_map(|(job_id, resource)| {
            let (outputs_in_custody, high_water) = resource.snapshot();
            (outputs_in_custody != 0).then_some(crate::NativeOutputCustody {
                original_scope_id: resource.scope_id,
                original_job_id: *job_id,
                outputs_in_custody,
                high_water,
            })
        })
        .collect()
}
