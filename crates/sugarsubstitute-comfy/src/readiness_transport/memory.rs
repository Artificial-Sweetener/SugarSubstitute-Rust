// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Partition the same readiness admission among HTTP/metadata, TCP snapshot and attached identity.
/// HTTP bodies, direct typed metadata, validation scratch, native structures and owner controls.
pub(super) const BODY_BYTES: u64 = 8 * 1024 * 1024;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryClaimError};
/// Keep the original body credit outside data/control storage, including escaped failures.
pub(super) struct BodyClaim(Option<Arc<MemoryClaim>>);
impl BodyClaim {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(BODY_BYTES)?;
        Ok(Self(Some(Arc::new(claim))))
    }
    #[cfg(test)]
    pub(super) fn bytes(&self) -> u64 {
        self.0.as_ref().map_or(0, |claim| claim.bytes())
    }
}
impl Clone for BodyClaim {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for BodyClaim {
    fn drop(&mut self) {
        if let Some(claim) = self.0.take() {
            // Final Arc control is deallocated before its returned MemoryClaim releases credit.
            drop(Arc::into_inner(claim));
        }
    }
}
const _: () = assert!(
    BODY_BYTES
        + sugarsubstitute_supervision::backend_listener::TCP_OWNER_WORKSPACE_BYTES
        + 64 * 1024
        == super::READINESS_TRANSPORT_BYTES
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readiness::{
        MAX_EXTENSION_PATHS, MAX_EXTENSIONS_BYTES, MAX_SYSTEM_STATS_BYTES, PackageVersion,
    };
    #[test]
    fn compiled_complete_phase_envelopes_fit_the_original_http_partition() {
        // Pinned RawVec doubles capacity; include both old and new storage during growth.
        let raw = MAX_SYSTEM_STATS_BYTES + MAX_EXTENSIONS_BYTES;
        let syntax_keys = MAX_EXTENSIONS_BYTES + 33 * 128 * size_of::<Option<String>>();
        let string_headers = MAX_EXTENSION_PATHS * size_of::<String>();
        let valid_stats = 4 * 1024
            + 64 * 3 * 256
            + MAX_SYSTEM_STATS_BYTES
            + 128 * (size_of::<PackageVersion>() + size_of::<String>());
        // Existing normalized configuration, WinHTTP header conversions/read stack, and owner/control cells.
        let controls = 512 * 1024
            + 3 * 32 * 1024
            + size_of::<super::super::owner::Transaction>()
            + size_of::<super::super::owner::FundedTransaction>()
            + 2 * size_of::<usize>()
            + size_of::<MemoryClaim>()
            + 16 * size_of::<sugarsubstitute_supervision::RegistryError>();
        let syntax_peak =
            raw + 3 * MAX_EXTENSIONS_BYTES + syntax_keys + string_headers + valid_stats + controls;
        // Each actual BTree node has parent/index/count, 11 keys and 12 child pointers.
        // Conservatively give every admitted path one complete internal node; include split overlap.
        let tree_node = 2 * size_of::<usize>() + 11 * size_of::<&str>() + 12 * size_of::<usize>();
        let comparison_peak = raw
            + 2 * MAX_EXTENSIONS_BYTES
            + 2 * string_headers
            + (MAX_EXTENSION_PATHS + 33) * tree_node
            + 2 * MAX_EXTENSION_PATHS * size_of::<&str>()
            + valid_stats
            + controls;
        // System stats has the smaller body but wrong textual types can format Debug escapes.
        let schema_peak = raw
            + 3 * MAX_SYSTEM_STATS_BYTES
            + MAX_SYSTEM_STATS_BYTES
            + 3 * MAX_EXTENSION_PATHS * (size_of::<PackageVersion>() + size_of::<String>())
            + 18 * MAX_SYSTEM_STATS_BYTES
            + controls;
        assert!(syntax_peak < BODY_BYTES as usize);
        assert!(comparison_peak < BODY_BYTES as usize);
        assert!(schema_peak < BODY_BYTES as usize);
        eprintln!(
            "readiness compiled owned envelopes: syntax={syntax_peak}, comparison={comparison_peak}, schema={schema_peak}, body={BODY_BYTES}"
        );
    }
}
