// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain this invocation's passive originals separately from first installation recovery bytes.
mod adoption;
mod manifest;
mod native;
mod publication;
mod reading;
mod settlement;
pub use settlement::SourceLaunchSettlement;
#[cfg(test)]
mod tests;
use super::controls::Controls;
use crate::durable_storage::{check, files::Directory, invalid};
pub(super) use native::VerifiedHost;
use std::{io, path::Path};
use sugarsubstitute_configuration::paths::StartupPaths;
use sugarsubstitute_contracts::{
    coordinator_entry::EntryOriginalsReference,
    execution::{JobContext, MemoryClaim},
};

const RETAINED_BYTES: u64 = 4 * 1024 * 1024;
/// Claim a source settlement keeps charged from its settlement until its owner drops.
const SETTLEMENT_BYTES: u64 = 8 * 1024 * 1024;
/// Memory the coordinator keeps charged for its whole life once launch data is settled: the
/// settled source's claim and the passive invocation custody retained through retirement.
pub(super) const LIFETIME_BYTES: u64 = SETTLEMENT_BYTES + RETAINED_BYTES;

/// Own every current-invocation attempt and final passive pin through actual handoff terminal.
pub(super) struct InvocationSnapshot {
    host: VerifiedHost,
    stage: Option<Directory>,
    final_files: Option<reading::PinnedOriginals>,
    reference: Option<EntryOriginalsReference>,
    _memory: MemoryClaim,
}
impl InvocationSnapshot {
    /// Read actual native producer provenance before any feature storage is created.
    pub(super) fn begin(image: &Path, context: &JobContext) -> io::Result<Self> {
        let memory = context.memory().claim(RETAINED_BYTES).map_err(io::Error::other)?;
        let host = VerifiedHost::capture(image, context)?;
        Ok(Self { host, stage: None, final_files: None, reference: None, _memory: memory })
    }
    /// Publish only captured raw source documents; no installation or Comfy authority is inferred.
    /// Interrupted stages and original bytes remain intact. The caller retains this owner on error.
    pub(super) fn publish(
        &mut self,
        controls: &Controls,
        paths: &StartupPaths,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        if self.stage.is_some() || self.final_files.is_some() || self.reference.is_some() {
            return Err(invalid("Invocation snapshot publication already began"));
        }
        publication::publish(self, controls, paths, context)
    }
    /// A reference escapes only after complete publication and final native/content validation.
    pub(super) fn reference(&self) -> Option<EntryOriginalsReference> {
        self.reference
    }
}
