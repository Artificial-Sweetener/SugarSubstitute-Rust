// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep exact recoverable policy outages distinct from independent failed native operations and cleanup.
use super::*;
/// A retained startup/request failure keeps its existing typed reply/settlement path.
/// Only the actual Backend owner's closed recovery classification permits managed-outage continuation.
pub(super) enum Disposition {
    Healthy,
    RequestFailed,
    Recoverable,
    FatalCleanup,
}
pub(super) fn classify(backend: &CoordinatorBackend, key: ResourceKey) -> Result<Disposition> {
    if backend.cleanup_failure(key).is_some() {
        return Ok(Disposition::FatalCleanup);
    }
    if backend.recoverable_failure(key).map_err(outer)?.is_some() {
        return Ok(Disposition::Recoverable);
    }
    Ok(if backend.failure(key).is_some() {
        Disposition::RequestFailed
    } else {
        Disposition::Healthy
    })
}
/// Marker refers to the original separately retained cleanup cause, never copies or clears it.
#[derive(Debug)]
pub(super) struct CleanupFailure {
    pub(super) resource: ResourceKey,
}
impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Actual backend cleanup failed for {:?}", self.resource)
    }
}
impl Error for CleanupFailure {}
/// A failed operation remains fatal even if that resource also owns a known managed outage.
#[derive(Debug)]
pub(super) struct OperationFailure {
    resource: ResourceKey,
    original: Box<dyn Error>,
}
impl OperationFailure {
    pub(super) fn new(resource: ResourceKey, original: Box<dyn Error>) -> Self {
        Self { resource, original }
    }
    pub(super) fn cause(&self) -> &(dyn Error + 'static) {
        self.original.as_ref()
    }
}
impl std::fmt::Display for OperationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Actual backend operation failed for {:?}", self.resource)
    }
}
impl Error for OperationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.original.as_ref())
    }
}
