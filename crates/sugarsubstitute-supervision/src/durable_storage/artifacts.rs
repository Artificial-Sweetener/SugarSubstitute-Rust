// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound caller-selected inputs and staged outputs without acquiring application ownership.
mod input;
#[cfg(windows)]
mod retained;
pub use input::{ArtifactInput, with_artifact_input};
#[cfg(windows)]
pub use retained::{ArtifactReadPermit, RetainedArtifactReader};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
#[allow(unsafe_code)]
mod portable;
use super::{check, control::effect, invalid};
#[cfg(not(windows))]
use portable as platform;
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Explicit publication rights for a caller-selected qualification artifact.
#[derive(Clone, Copy)]
pub enum ArtifactPublication {
    /// Preserve every occupied destination.
    CreateOnly,
    /// Replace the selected leaf atomically after its complete stage is flushed and closed.
    ReplaceSelected,
}

/// Allocate a canonical lowercase UUID on an admitted Windows storage worker.
/// This creates correlation only; it grants no installation, file or process authority.
/// # Errors
/// Preserves original cancellation/deadline and operating-system generation failures.
#[cfg(windows)]
pub fn new_artifact_identity(context: &JobContext) -> io::Result<String> {
    check(context)?;
    let value = super::staging::identity()?;
    check(context)?;
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    ))
}

/// Bounded input bytes coupled to their actual original execution allocation.
pub struct ArtifactBytes {
    bytes: Vec<u8>,
    _memory: MemoryClaim,
}
impl ArtifactBytes {
    /// Borrow the selected bytes while their memory reservation remains retained.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

/// Read one explicitly selected input, including supported read-only filesystem links.
/// Runs synchronously on the admitted worker. Its byte bound includes an overflow sentinel;
/// no handle escapes and the returned allocation retains the same job's memory charge.
/// # Errors
/// Preserves native and control causes and rejects excessive bytes or insufficient admission.
pub fn read_artifact(
    path: &Path,
    maximum: usize,
    context: &JobContext,
) -> io::Result<ArtifactBytes> {
    check(context)?;
    let capacity =
        maximum.checked_add(1).ok_or_else(|| invalid("Artifact input bound overflow"))?;
    let memory = context.memory().claim(capacity as u64).map_err(io::Error::other)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(io::Error::other)?;
    if bytes.capacity() != capacity {
        return Err(invalid("Artifact input allocation exceeds admission"));
    }
    bytes.resize(capacity, 0);
    let mut used = 0;
    with_artifact_input(path, maximum as u64, context, |file| {
        loop {
            let count = file.read(&mut bytes[used..])?;
            if count == 0 {
                break;
            }
            used += count;
            if used > maximum {
                return Err(invalid("Artifact input exceeds its byte bound"));
            }
        }
        Ok::<(), io::Error>(())
    })
    .map_err(|cause| match cause {
        ArtifactError::Storage(cause) | ArtifactError::Producer(cause) => cause,
        ArtifactError::Retirement { producer, storage } => retirement_error(producer, Err(storage)),
    })?;
    bytes.truncate(used);
    Ok(ArtifactBytes { bytes, _memory: memory })
}

/// Failure retains producer and native retirement causes independently.
#[derive(Debug)]
pub enum ArtifactError<E> {
    /// Native acquisition, writing, publication or original controls failed.
    Storage(io::Error),
    /// The feature's encoder or schema producer failed before publication.
    Producer(E),
    /// Producer failure was followed by an original-control or checked file-close failure.
    Retirement {
        /// Original feature failure.
        producer: E,
        /// Control/native retirement failure; successful completion is not implied.
        storage: io::Error,
    },
}
impl<E: std::fmt::Display> std::fmt::Display for ArtifactError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(cause) => write!(formatter, "Artifact storage failed: {cause}"),
            Self::Producer(cause) => write!(formatter, "Artifact production failed: {cause}"),
            Self::Retirement { producer, storage } => {
                write!(formatter, "{producer}; artifact retirement failed: {storage}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ArtifactError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(cause) => Some(cause),
            Self::Producer(cause) | Self::Retirement { producer: cause, .. } => Some(cause),
        }
    }
}

/// Produce one bounded artifact on its original admitted worker, never on an active UI thread.
/// The feature supplies schema/encoding, explicit destination rights and parent-creation intent.
/// Writes stay in a fresh sibling until flush, checked native close and final control checks
/// succeed. Failed stages remain inert; this API grants no managed-data or cleanup authority.
/// # Errors
/// Preserves producer/native failures, refuses links or ambiguous output ancestry, and rejects
/// bytes beyond `maximum`. Cancellation cannot activate an unfinished stage or truncate a target.
pub fn write_artifact<E>(
    path: &Path,
    create_parents: bool,
    publication: ArtifactPublication,
    maximum: usize,
    context: &JobContext,
    produce: impl FnOnce(&mut dyn Write) -> Result<(), E>,
) -> Result<(), ArtifactError<E>> {
    check(context).map_err(ArtifactError::Storage)?;
    if maximum == 0 {
        return Err(ArtifactError::Storage(invalid("Artifact output requires a byte bound")));
    }
    let mut stage =
        platform::Stage::open(path, create_parents, context).map_err(ArtifactError::Storage)?;
    let result = produce(&mut Writer { stage: &mut stage, context, remaining: maximum });
    if let Err(producer) = result {
        return match stage.abort() {
            Ok(()) => Err(ArtifactError::Producer(producer)),
            Err(storage) => Err(ArtifactError::Retirement { producer, storage }),
        };
    }
    if let Err(cause) = effect(context, || stage.flush()) {
        return Err(ArtifactError::Storage(retirement_error(cause, stage.abort())));
    }
    if let Err(cause) = stage.close().and_then(|()| check(context)) {
        return Err(ArtifactError::Storage(retirement_error(cause, stage.abort())));
    }
    stage.publish(publication, context).map_err(ArtifactError::Storage)
}

/// Preserve the first native/control result together with a later close failure.
fn retirement_error(primary: io::Error, retired: io::Result<()>) -> io::Error {
    match retired {
        Ok(()) => primary,
        Err(retirement) => {
            io::Error::new(primary.kind(), RetirementFailure { primary, retirement })
        }
    }
}
/// Join native retirement without losing the preceding result.
fn finish_io(result: io::Result<()>, retired: io::Result<()>) -> io::Result<()> {
    match (result, retired) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
        (Err(primary), Err(retirement)) => {
            Err(io::Error::new(primary.kind(), RetirementFailure { primary, retirement }))
        }
    }
}
#[derive(Debug)]
struct RetirementFailure {
    primary: io::Error,
    retirement: io::Error,
}
impl std::fmt::Display for RetirementFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}; artifact close failed: {}", self.primary, self.retirement)
    }
}
impl std::error::Error for RetirementFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}

/// Check controls around each physical write; the producer cannot detach native capabilities.
struct Writer<'a, 'b> {
    stage: &'a mut platform::Stage,
    context: &'b JobContext,
    remaining: usize,
}
impl Write for Writer<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(invalid("Artifact output exceeds its byte bound"));
        }
        let count = effect(self.context, || self.stage.write(bytes))?;
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        effect(self.context, || self.stage.flush())
    }
}

/// Resolve explicit relative output intent before validating bounded, unambiguous ancestry.
fn output_path(path: &Path) -> io::Result<PathBuf> {
    use std::path::Component;
    let path =
        if path.is_absolute() { path.to_owned() } else { std::env::current_dir()?.join(path) };
    if path.as_os_str().len() > 4096
        || path.components().count() > 64
        || path.components().any(|part| matches!(part, Component::ParentDir))
        || path.file_name().is_none()
    {
        return Err(invalid("Artifact output path is ambiguous or exceeds its bound"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests;
