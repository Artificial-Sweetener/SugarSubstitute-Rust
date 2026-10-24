// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode setup's producing distribution on its admitted worker.
use std::io;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::setup_storage::InstallerDistribution;

/// Decode on an admitted worker; a build without distribution authority cannot install payloads.
pub(super) fn decode(bytes: &[u8], context: &JobContext) -> io::Result<InstallerDistribution> {
    InstallerDistribution::decode(bytes, context)
}
