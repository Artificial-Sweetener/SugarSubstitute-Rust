// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate immutable fixture identities before any public capture effect.
use super::*;
/// Reserve the actual handler observation before start is admitted.
pub(super) const WATCH_JOB: u64 = 1;
/// Keep registration distinct from all bounded close identities.
pub(super) const START_JOB: u64 = 2;
/// Admit finite lifecycle work with a caller-owned immutable deadline and charge.
pub(super) fn finite(id: u64, deadline: Moment, bytes: u64) -> Result<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(deadline),
        reservation_bytes: bytes,
        replacement: None,
    })
}
/// Keep actual handler observation alive independently of registration cancellation.
pub(super) fn watch() -> Result<JobSpec> {
    let mut spec = finite(WATCH_JOB, Moment::from_nanos(1), 4096)?;
    spec.class = JobClass::NativeObservation;
    spec.deadline = None;
    Ok(spec)
}
