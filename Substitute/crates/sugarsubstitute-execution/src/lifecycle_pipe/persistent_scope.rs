// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original finite handshakes independently of persistent native retirement.
use super::*;
use crate::windows_lifecycle_pipe::LifecyclePipeAdmissionError;

/// Reject a second persistent-job deadline before native transport construction.
/// Actual scope ownership/liveness remains with the central admission transaction.
pub(crate) fn validate_persistent_scope(
    core: &Core,
    scope: &Scope,
    spec: JobSpec,
) -> Result<(), LifecyclePipeAdmissionError> {
    if spec.deadline.is_some() {
        return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
    }
    let state = lock(&core.state);
    if state.scopes.get(&scope.id()).is_some_and(|scope| scope.deadline.is_some()) {
        return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
    }
    Ok(())
}
