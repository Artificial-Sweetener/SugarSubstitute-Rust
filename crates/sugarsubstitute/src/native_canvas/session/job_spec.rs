// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind canvas work to the shared job identity sequence, with or without a cutoff of its own.
use super::*;

impl Session {
    /// Bind work to one original deadline and the same sole job identity sequence. Use this only
    /// for work whose callees require an original deadline, such as storage.
    pub(super) fn spec(
        &self,
        operation: Operation,
        class: JobClass,
        bytes: u64,
        duration: Duration,
        replacement: Option<ReplacementKey>,
    ) -> Result<JobSpec, Box<dyn Error>> {
        Ok(JobSpec {
            id: self.sequence.allocate()?,
            operation,
            class,
            deadline: Some(self.clock.now().checked_add(duration)?),
            reservation_bytes: bytes,
            replacement,
        })
    }

    /// Bind work that ends on its own result, an observed failure or cancellation. It carries no
    /// cutoff of its own: the execution scope's shutdown cutoff alone bounds it, so a host that
    /// is slow under load delays it instead of failing it.
    pub(super) fn event_spec(
        &self,
        operation: Operation,
        class: JobClass,
        bytes: u64,
        replacement: Option<ReplacementKey>,
    ) -> Result<JobSpec, Box<dyn Error>> {
        Ok(JobSpec {
            id: self.sequence.allocate()?,
            operation,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement,
        })
    }
}
