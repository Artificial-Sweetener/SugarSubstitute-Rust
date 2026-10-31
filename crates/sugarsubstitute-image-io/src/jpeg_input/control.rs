// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe controls outside decoder buffering and retain failures swallowed by bytewise reads.
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use sugarsubstitute_contracts::execution::JobContext;

/// Retain the first control or storage failure through the original worker lifetime.
struct Controls<'a> {
    context: &'a JobContext,
    failure: Option<io::Error>,
}
impl Controls<'_> {
    /// A cancelled or expired input never becomes readable again.
    fn check(&mut self) -> io::Result<()> {
        if self.failure.is_none() {
            let kind = if self.context.cancellation_requested() {
                Some(io::ErrorKind::Interrupted)
            } else if self.context.deadline().is_some_and(|limit| self.context.now() >= limit) {
                Some(io::ErrorKind::TimedOut)
            } else {
                None
            };
            self.failure =
                kind.map(|kind| io::Error::new(kind, "JPEG input controls stopped decoding"));
        }
        match &self.failure {
            Some(cause) => {
                Err(io::Error::new(cause.kind(), "JPEG input stopped after an earlier failure"))
            }
            None => Ok(()),
        }
    }
    /// Keep original storage causes even when the decoder discards a speculative read error.
    fn observe<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Ok(value) => {
                self.check()?;
                Ok(value)
            }
            Err(cause) => {
                let kind = cause.kind();
                if self.failure.is_none() {
                    self.failure = Some(cause);
                }
                Err(io::Error::new(kind, "JPEG input failed"))
            }
        }
    }
    /// Return the retained cause instead of its lightweight sticky rejection.
    fn finish(mut self) -> io::Result<()> {
        let observed = self.check();
        match self.failure {
            Some(cause) => Err(cause),
            None => observed,
        }
    }
}

/// Keep checks outside every buffered read, seek and EOF observation.
pub(super) struct CheckedInput<'a, R> {
    inner: R,
    controls: Controls<'a>,
}
impl<'a, R> CheckedInput<'a, R> {
    /// Borrow the original job; this wrapper owns no scheduler or independent deadline.
    pub(super) fn new(inner: R, context: &'a JobContext) -> Self {
        Self { inner, controls: Controls { context, failure: None } }
    }
    /// Observe any sticky failure before releasing the original stream.
    pub(super) fn finish(self) -> io::Result<R> {
        self.controls.finish()?;
        Ok(self.inner)
    }
}
impl<R: Read> Read for CheckedInput<'_, R> {
    fn read_exact(&mut self, mut output: &mut [u8]) -> io::Result<()> {
        // std's default retries Interrupted; a sticky job cancellation must stop immediately.
        while !output.is_empty() {
            let read = self.read(output)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "JPEG input ended before the exact read",
                ));
            }
            output = &mut output[read..];
        }
        Ok(())
    }
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.controls.check()?;
        let result = self.inner.read(output);
        self.controls.observe(result)
    }
}
impl<R: BufRead> BufRead for CheckedInput<'_, R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.controls.check()?;
        let result = self.inner.fill_buf();
        self.controls.observe(result)
    }
    fn consume(&mut self, amount: usize) {
        if self.controls.check().is_ok() {
            self.inner.consume(amount);
        }
    }
}
impl<R: Seek> Seek for CheckedInput<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.controls.check()?;
        let result = self.inner.seek(from);
        self.controls.observe(result)
    }
}
