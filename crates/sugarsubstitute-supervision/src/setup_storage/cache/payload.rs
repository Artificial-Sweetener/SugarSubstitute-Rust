// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund resumable file custody from its storage grant's original allocation reservation.
use super::*;
use crate::durable_storage::payload::Payload;

impl CacheLease {
    /// Open one declared resumable file under this exclusive cache grant.
    /// The payload retains independent native parent custody and its original cache allocation;
    /// the caller retains this lease until all file and native consumers retire. Current jobs
    /// fund transient copying and protection only. No allocation lease or writable file escapes.
    /// Rejects undeclared names, readers, generated-output grants and excessive extents before I/O.
    pub(crate) fn payload(
        &self,
        name: &str,
        maximum: u64,
        context: &JobContext,
    ) -> io::Result<Payload> {
        check(context)?;
        self.require_declared_writer()?;
        if !self.record.files.iter().any(|declared| declared == name)
            || maximum > self.record.reserved
        {
            return Err(invalid("Resumable payload exceeds its cache declaration"));
        }
        Payload::open(&self._data, name, maximum, &self._cache.scope.memory, context)
    }
}

#[cfg(test)]
mod tests;
