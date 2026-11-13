// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve immutable output identities while a later capture advances workspace presentation.
use super::*;
use std::{collections::BTreeMap, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;
/// Constant-time owned loan of the preceding durable checkpoint, retaining its original charge.
pub struct CheckpointBaseline {
    pub(super) restored: Option<Arc<RestoredWorkspace>>,
    pub(super) published: Option<Arc<CapturedWorkspace>>,
}
impl CheckpointBaseline {
    /// Borrow only assets already admitted by this exact durable workspace namespace. A
    /// matching digest elsewhere in the profile's deduplicated directory grants no adoption.
    pub fn retained_assets(
        &self,
    ) -> &[sugarsubstitute_contracts::desktop_session::wire::SessionAsset] {
        self.previous().map_or(&[], |checkpoint| checkpoint.assets.as_slice())
    }
    fn previous(&self) -> Option<&SessionCheckpoint> {
        self.published.as_ref().map(|value| &value.checkpoint).or_else(|| {
            self.restored
                .as_ref()
                .and_then(|value| value.checkpoint.as_ref().map(|(_, checkpoint)| checkpoint))
        })
    }
    /// Compare retained output identities on the publication worker before any new asset or
    /// archive I/O. Presentation changes may remove/reorder outputs, but cannot change a known
    /// output UUID's immutable bytes, provenance, file descriptor or dimensions.
    /// # Errors
    /// Refuses changed known identities and original cancellation/deadline/memory limits.
    pub fn verify(
        &self,
        captured: &SessionCheckpoint,
        context: &JobContext,
    ) -> Result<(), StoreError> {
        let Some(previous) = self.previous() else {
            return Ok(());
        };
        if previous.scope != captured.scope {
            return Err(StoreError::Protected);
        }
        let count = previous.entries.iter().map(|entry| entry.outputs.len()).sum::<usize>();
        let _memory =
            context.memory().claim((count as u64).saturating_mul(128)).map_err(io::Error::other)?;
        let mut outputs = BTreeMap::new();
        for output in previous.entries.iter().flat_map(|entry| &entry.outputs) {
            check(context)?;
            outputs.insert(&output.id, output);
        }
        for output in captured.entries.iter().flat_map(|entry| &entry.outputs) {
            check(context)?;
            if outputs.get(&output.id).is_some_and(|previous| *previous != output) {
                return Err(StoreError::Changed);
            }
        }
        Ok(())
    }
}
fn check(context: &JobContext) -> Result<(), StoreError> {
    if context.cancellation_requested() {
        return Err(io::Error::from(io::ErrorKind::Interrupted).into());
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(io::Error::from(io::ErrorKind::TimedOut).into());
    }
    Ok(())
}
