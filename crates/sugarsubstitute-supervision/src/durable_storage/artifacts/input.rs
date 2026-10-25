// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow selected regular input files without buffering their complete contents.
use super::{ArtifactError, check, effect, finish_io, invalid, platform, retirement_error};
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};
use sugarsubstitute_contracts::execution::JobContext;

/// A selected regular file borrowed only during its original admitted storage operation.
/// Reads and seeks check cancellation/deadline before and after native effects. The initial
/// byte length bounds every read and seek; buffering belongs to the caller's execution admission.
/// This is read authority only: it grants no parent, mutation, cache or cleanup ownership.
pub struct ArtifactInput<'a> {
    file: &'a mut platform::Input,
    context: &'a JobContext,
    length: u64,
    position: u64,
}
impl ArtifactInput<'_> {
    /// Regular-file length observed before the producer receives read authority, in bytes.
    pub const fn len(&self) -> u64 {
        self.length
    }
    /// Whether the selected file was empty at acquisition.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}
impl Read for ArtifactInput<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        check(self.context)?;
        let available = self.length - self.position;
        let count = output.len().min(usize::try_from(available).unwrap_or(usize::MAX));
        if count == 0 {
            return Ok(0);
        }
        let read = effect(self.context, || self.file.read(&mut output[..count]))?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Selected input changed length",
            ));
        }
        self.position += read as u64;
        Ok(read)
    }
}
impl Seek for ArtifactInput<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        check(self.context)?;
        let offset = match position {
            SeekFrom::Start(value) => Some(value),
            SeekFrom::Current(value) => self.position.checked_add_signed(value),
            SeekFrom::End(value) => self.length.checked_add_signed(value),
        }
        .filter(|value| *value <= self.length)
        .ok_or_else(|| invalid("Selected input seek exceeds its original extent"))?;
        let actual = effect(self.context, || self.file.seek(SeekFrom::Start(offset)))?;
        if actual != offset {
            return Err(invalid("Selected input seek returned another position"));
        }
        self.position = actual;
        Ok(actual)
    }
}

/// Process one explicitly selected regular file on its admitted storage worker.
/// The callback borrows a seekable stream, not a path or detachable native handle. Read-only
/// filesystem links are supported. Windows sharing denies writes/replacement while open;
/// portable callers must verify content identities when external writers are possible.
/// No image-sized allocation is made here. The native handle keeps its existing 64 KiB charge
/// until checked close; callback results retain their own original allocation claims.
/// # Errors
/// Rejects nonregular or oversized inputs before invoking the callback, observes original
/// controls, and preserves callback failures together with later control/close failures.
pub fn with_artifact_input<T, E>(
    path: &Path,
    maximum: u64,
    context: &JobContext,
    consume: impl FnOnce(&mut ArtifactInput<'_>) -> Result<T, E>,
) -> Result<T, ArtifactError<E>> {
    check(context).map_err(ArtifactError::Storage)?;
    let mut file = platform::Input::open(path, context).map_err(ArtifactError::Storage)?;
    let length = match effect(context, || file.length()).and_then(|length| {
        if length > maximum {
            Err(invalid("Selected input exceeds its byte bound"))
        } else {
            Ok(length)
        }
    }) {
        Ok(length) => length,
        Err(cause) => return Err(ArtifactError::Storage(retirement_error(cause, file.close()))),
    };
    let result = consume(&mut ArtifactInput { file: &mut file, context, length, position: 0 });
    let retired = finish_io(check(context), file.close());
    match (result, retired) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cause)) => Err(ArtifactError::Storage(cause)),
        (Err(cause), Ok(())) => Err(ArtifactError::Producer(cause)),
        (Err(producer), Err(storage)) => Err(ArtifactError::Retirement { producer, storage }),
    }
}
