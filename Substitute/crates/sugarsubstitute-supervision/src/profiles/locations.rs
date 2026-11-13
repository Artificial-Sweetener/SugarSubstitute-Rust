// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the native profile lifecycle controls independently of copied browser data.
use sugarsubstitute_contracts::lifecycle::RunKey;
pub(crate) const CLAIM: &str = ".sugarsubstitute-profile-v1.claim";
pub(crate) const RECORD: &str = ".sugarsubstitute-profile-v1.record";
pub(crate) const ORIGINAL_CLAIM: &str = ".sugarsubstitute-profile-v1.original.claim";
pub(crate) const ORIGINAL_RECORD: &str = ".sugarsubstitute-profile-v1.original.record";
pub(crate) const CUSTODY: &str = ".sugarsubstitute-profile-v2.custody";
pub(crate) const RECOVERED_PREFIX: &str = ".sugarsubstitute-profile-recovered-";
pub(crate) const CONTROL_PREFIX: &str = ".sugarsubstitute-profile-";

/// Exact recovery-pair stem bound to the authoritative row's original run.
pub(crate) fn recovered(run: RunKey) -> String {
    let epoch =
        run.owner.epoch.bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    format!("{RECOVERED_PREFIX}{epoch}-{}-{}", run.instance.get(), run.generation.get())
}
