// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Journal feature-requested retirement while shared storage owns actual safe reclamation.
use super::*;
impl CacheLease {
    /// Retire this exact disposable generation after the feature proves every native writer stopped.
    /// This records intent only. Existing file custody wins over cleanup until its actual release.
    /// The shared maintenance owner validates and removes known files after this lease is dropped.
    /// # Errors
    /// Rejects readers, pending native production, unknown files, cancellation and storage failures.
    pub fn retire(&mut self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.require_declared_writer()?;
        if self.record.deleting {
            return Ok(());
        }
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let _writer =
            protection::with(context, |protection| self._cache.writer(protection, context))?;
        if consumers::live(&self.directory, &self.record, context)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Cache native consumers remain live",
            ));
        }
        let mut record = self.record.clone();
        record.deleting = true;
        tree::inspect(&self.directory, &record, context)?;
        let parent = self._cache.namespace(record.namespace)?;
        record::start_retirement(&parent, &record, context).map_err(record::public_error)?;
        self.record = record;
        Ok(())
    }
}
