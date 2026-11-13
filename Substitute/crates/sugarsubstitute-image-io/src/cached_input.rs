// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound an already-pinned encoded input without reopening a filesystem path.

use std::io::{self, Read, Seek, SeekFrom};
use sugarsubstitute_contracts::execution::JobContext;

/// Limit reads and seeks to the original size while checking the owning job's controls.
pub(super) struct BoundedInput<'a, R> {
    input: &'a mut R,
    context: &'a JobContext,
    length: u64,
    position: u64,
}

impl<'a, R: Read + Seek> BoundedInput<'a, R> {
    /// Capture a bounded source length and begin at the first byte.
    pub(super) fn new(input: &'a mut R, maximum: u64, context: &'a JobContext) -> io::Result<Self> {
        check(context)?;
        let length = input.seek(SeekFrom::End(0))?;
        if length > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Encoded image exceeds its byte bound",
            ));
        }
        if input.seek(SeekFrom::Start(0))? != 0 {
            return Err(io::Error::other("Encoded image could not rewind"));
        }
        check(context)?;
        Ok(Self { input, context, length, position: 0 })
    }

    /// Reject a changed source length before releasing the caller's original pin.
    pub(super) fn finish(self) -> io::Result<()> {
        check(self.context)?;
        if self.input.seek(SeekFrom::End(0))? != self.length {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Encoded image length changed"));
        }
        check(self.context)
    }
}

impl<R: Read + Seek> Read for BoundedInput<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        check(self.context)?;
        let available = self.length - self.position;
        let count = usize::try_from(available.min(output.len() as u64))
            .map_err(|_| io::Error::other("Encoded image read bound overflow"))?;
        if count == 0 {
            return Ok(0);
        }
        let read = self.input.read(&mut output[..count])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Encoded image was truncated",
            ));
        }
        self.position += read as u64;
        check(self.context)?;
        Ok(read)
    }
}

impl<R: Read + Seek> Seek for BoundedInput<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        check(self.context)?;
        let target = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.length) + i128::from(offset),
        };
        if !(0..=i128::from(self.length)).contains(&target) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Encoded image seek exceeds its bound",
            ));
        }
        let target = u64::try_from(target).map_err(io::Error::other)?;
        if self.input.seek(SeekFrom::Start(target))? != target {
            return Err(io::Error::other("Encoded image seek returned a different position"));
        }
        self.position = target;
        check(self.context)?;
        Ok(target)
    }
}

/// Keep cancellation and the original deadline attached to every storage operation.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Encoded image read cancelled"));
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Encoded image read deadline expired"));
    }
    Ok(())
}
