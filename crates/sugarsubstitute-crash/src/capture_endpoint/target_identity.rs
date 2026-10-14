// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact full target correlation inside the original shared64-slot inventory.
use sugarsubstitute_contracts::crash::CaptureTarget;
/// The retained coordinator owner supplies the namespace and epoch.
/// Positive generation means Desktop; zero means the genuine BackendCustody session.
/// No RunKey or incident subject is constructed from this representation.
pub(super) struct TargetIdentity {
    pub(super) instance: u64,
    pub(super) generation: u64,
}
impl TargetIdentity {
    pub(super) fn new(target: CaptureTarget) -> Self {
        match target {
            CaptureTarget::Desktop(run) => {
                Self { instance: run.instance.get(), generation: run.generation.get() }
            }
            CaptureTarget::BackendCustody(key) => {
                Self { instance: key.session().get(), generation: 0 }
            }
        }
    }
    pub(super) fn matches(&self, instance: u64, generation: u64) -> bool {
        self.instance == instance && self.generation == generation
    }
}
