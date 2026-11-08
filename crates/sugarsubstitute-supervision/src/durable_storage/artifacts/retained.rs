// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse one immutable regular-file read capability across fresh finite worker operations.
use super::*;
use std::io::{Seek, SeekFrom};
use sugarsubstitute_contracts::execution::{ExecutionId, JobClass, Operation};

/// One exact regular file retained against writes/replacement, without exporting its path or
/// native handle. Hash/format authority belongs to its feature owner. Acquire, borrow, and
/// final checked close only on admitted storage workers from the same execution owner.
///
/// This object retains allocation custody, not an expired JobContext. Later borrows use their
/// own original deadline/cancellation. The composing owner bounds registry/concurrency and
/// must retain this object until [`Self::close`] actually succeeds.
pub struct RetainedArtifactReader {
    input: super::windows::RetainedInput,
    owner: ExecutionId,
    length: u64,
    position: u64,
    closed: bool,
}
impl RetainedArtifactReader {
    /// Acquire an immutable, bounded regular file under this ordinary worker's native custody.
    /// The source owner must verify complete digest/length before making it presentable.
    /// `memory` funds the retained 64 KiB native handle independently of transient range
    /// response storage, so retaining a reader does not retain each large read-job reservation.
    /// # Errors
    /// Refuses nonordinary/unbounded/ended work, nonregular/oversized inputs and native failures.
    pub fn open(
        path: &Path,
        maximum: u64,
        memory: &sugarsubstitute_contracts::execution::MemoryLease,
        context: &JobContext,
    ) -> io::Result<Self> {
        ordinary(context)?;
        let mut input = super::windows::RetainedInput::open(path, memory, context)?;
        let result = input.length().and_then(|length| {
            if length == 0 || length > maximum {
                return Err(invalid("Retained artifact extent exceeds its admission"));
            }
            check(context)?;
            Ok(length)
        });
        match result {
            Ok(length) => {
                Ok(Self { input, owner: context.owner_id(), length, position: 0, closed: false })
            }
            Err(cause) => Err(retirement_error(cause, input.close())),
        }
    }
    /// Immutable regular-file length established on its original retained handle, in bytes.
    pub const fn len(&self) -> u64 {
        self.length
    }
    /// Whether this retained source was empty. Acquisition rejects empty sources.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    /// Lend bounded Read+Seek through the current job's fresh controls. The handle and original
    /// immutable native object remain retained; no reopening or rehashing is implied.
    /// # Errors
    /// Refuses a closed/foreign owner, nonordinary or ended work; preserves producer/control causes.
    pub fn with_reader<T, E>(
        &mut self,
        context: &JobContext,
        consume: impl FnOnce(&mut ArtifactReadPermit<'_>) -> Result<T, E>,
    ) -> Result<T, ArtifactError<E>> {
        ordinary(context).map_err(ArtifactError::Storage)?;
        self.admit(context).map_err(ArtifactError::Storage)?;
        let result = consume(&mut ArtifactReadPermit { reader: self, context });
        match (result, check(context)) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(cause)) => Err(ArtifactError::Storage(cause)),
            (Err(cause), Ok(())) => Err(ArtifactError::Producer(cause)),
            (Err(producer), Err(storage)) => Err(ArtifactError::Retirement { producer, storage }),
        }
    }
    /// Observe the original checked native close on an ordinary/lifecycle cleanup worker.
    /// Failure retains its handle/allocation; retry with fresh same-owner cleanup admission.
    /// # Errors
    /// Preserves foreign owner, original controls and actual close failures.
    pub fn close(&mut self, context: &JobContext) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if context.owner_id() != self.owner
            || context.spec().operation != Operation::Infrastructure
            || context.deadline().is_none()
        {
            return Err(invalid("Retained artifact cleanup belongs to another owner"));
        }
        check(context)?;
        self.input.close()?;
        self.closed = true;
        check(context)
    }
    /// Whether the exact original file handle's checked close succeeded.
    pub const fn closed(&self) -> bool {
        self.closed
    }
    fn admit(&self, context: &JobContext) -> io::Result<()> {
        if self.closed || context.owner_id() != self.owner {
            return Err(invalid("Retained artifact belongs to a closed or foreign owner"));
        }
        check(context)
    }
}
/// A synchronous, non-detachable Read+Seek loan scoped to one admitted finite worker callback.
pub struct ArtifactReadPermit<'a> {
    reader: &'a mut RetainedArtifactReader,
    context: &'a JobContext,
}
impl ArtifactReadPermit<'_> {
    /// Original immutable source length in bytes.
    pub const fn len(&self) -> u64 {
        self.reader.length
    }
    /// Whether the source was empty; acquisition rejects empty sources.
    pub const fn is_empty(&self) -> bool {
        self.reader.length == 0
    }
}
impl Read for ArtifactReadPermit<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reader.admit(self.context)?;
        let available = self.reader.length - self.reader.position;
        let maximum = output.len().min(usize::try_from(available).unwrap_or(usize::MAX));
        if maximum == 0 {
            return Ok(0);
        }
        let read = effect(self.context, || self.reader.input.read(&mut output[..maximum]))?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Retained artifact changed length",
            ));
        }
        self.reader.position += read as u64;
        Ok(read)
    }
}
impl Seek for ArtifactReadPermit<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.reader.admit(self.context)?;
        let offset = match position {
            SeekFrom::Start(value) => Some(value),
            SeekFrom::Current(value) => self.reader.position.checked_add_signed(value),
            SeekFrom::End(value) => self.reader.length.checked_add_signed(value),
        }
        .filter(|value| *value <= self.reader.length)
        .ok_or_else(|| invalid("Retained seek exceeds original extent"))?;
        let actual = effect(self.context, || self.reader.input.seek(SeekFrom::Start(offset)))?;
        if actual != offset {
            return Err(invalid("Retained seek returned another extent"));
        }
        self.reader.position = actual;
        Ok(actual)
    }
}
fn ordinary(context: &JobContext) -> io::Result<()> {
    if context.spec().class != JobClass::InfrastructureOrdinary
        || context.spec().operation != Operation::Infrastructure
        || context.deadline().is_none()
    {
        return Err(invalid("Retained artifact requires a finite ordinary storage job"));
    }
    check(context)
}
