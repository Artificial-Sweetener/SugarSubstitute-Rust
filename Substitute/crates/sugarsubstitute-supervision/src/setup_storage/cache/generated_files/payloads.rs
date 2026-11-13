// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native payloads while aggregate charge includes every active writer's admitted extent.
use super::*;
impl GeneratedFiles {
    /// Admit a resumable payload without resetting existing bytes. Only eight writers may coexist.
    pub(in crate::setup_storage) fn open(
        &mut self,
        name: &str,
        expected: Option<u64>,
        context: &JobContext,
    ) -> io::Result<u64> {
        self.check(context)?;
        if self.payloads.len() >= 8 || self.payloads.contains_key(name) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Download writer admission exhausted",
            ));
        }
        let reserved = self.index.charge(name)?.unwrap_or(0).max(expected.unwrap_or(0));
        self.index.admit(name, false, reserved)?;
        let parent = self.parent(name, true, context)?;
        let leaf = name.rsplit('/').next().ok_or_else(|| invalid("Download filename absent"))?;
        let payload = Payload::open(
            &parent,
            leaf,
            self.lease.reserved_bytes(),
            &self.lease._cache.scope.memory,
            context,
        )?;
        let length = payload.length()?;
        self.index.admit(name, false, reserved.max(length))?;
        self.payloads.insert(name.to_owned(), payload);
        Ok(length)
    }
    /// Explicit feature invalidation resets only this retained payload and its actual charge.
    pub(in crate::setup_storage) fn reset(
        &mut self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<()> {
        self.check(context)?;
        self.payloads.get_mut(name).ok_or_else(|| invalid("Download writer absent"))?.reset(context)
    }
    /// Charge all queued bytes before native append; failed/partial writes keep their high water.
    pub(in crate::setup_storage) fn append(
        &mut self,
        name: &str,
        offset: u64,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.check(context)?;
        let payload =
            self.payloads.get_mut(name).ok_or_else(|| invalid("Download writer absent"))?;
        if bytes.len() > 256 * 1024 || payload.length()? != offset {
            return Err(invalid("Download chunk does not match its physical prefix"));
        }
        let extent = offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("Download extent overflowed"))?;
        let reserved =
            self.index.charge(name)?.ok_or_else(|| invalid("Download admission absent"))?;
        self.index.admit(name, false, reserved.max(extent))?;
        payload.append(offset, bytes, context).map_err(io::Error::other)
    }
    /// Flush before releasing the native handle. Failure retains both custody and conservative bytes.
    /// Cleanup is permitted after cancellation; it cannot append, reset or publish content.
    pub(in crate::setup_storage) fn close(&mut self, name: &str) -> io::Result<()> {
        let payload = self.payloads.get(name).ok_or_else(|| invalid("Download writer absent"))?;
        payload.flush()?;
        self.index.admit(name, false, payload.length()?)?;
        self.payloads.remove(name);
        Ok(())
    }
}
