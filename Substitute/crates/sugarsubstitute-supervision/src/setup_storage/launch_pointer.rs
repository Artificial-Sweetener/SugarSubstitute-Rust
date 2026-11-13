// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the generation whose launch engine the stable entry starts, and one fallback.
//!
//! The pointer is the stable entry's only input and is permanent cross-generation surface: a
//! root executable of any age must read a pointer written by any later generation. Version 1 is
//! final. Later releases may add fields, which every reader ignores; they never rename, remove or
//! reinterpret a field, so the schema number never changes (`native-launch-pointer-v1`).
//!
//! Each target names a generation folder and the exact SHA-256 and size of that generation's
//! `sugarsubstitute-launcher.exe`. The root starts an engine only after its bytes match on a
//! handle that denies writes and deletion until the process exists. That is the root's whole
//! trust decision, and it is sufficient: the engine bytes were fully verified under signed
//! update metadata before a generation could be prepared, the pointer lives in the installation's
//! protected state folder, and anyone who can rewrite that folder can already replace the root
//! executable itself. The root therefore never reads selection, trust or update metadata.
//!
//! Only the installer and an engine redirecting startup to the generation that should run write
//! the pointer, always by atomic protected replacement.
use super::*;
use crate::durable_storage::{
    files::{self, Directory},
    protection::{self, Protection},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::installation::GenerationId;

/// Pointer file inside the installation's protected state folder.
pub const LAUNCH_POINTER_FILE: &str = "launch-pointer-v1.json";
/// Launch engine image inside each generation folder.
pub const LAUNCH_ENGINE: &str = "sugarsubstitute-launcher.exe";
/// Reserve on the admitted worker that reads the pointer and pins one engine at a time.
pub const POINTER_RESERVATION_BYTES: u64 = 2 * 1024 * 1024;
const SCHEMA: u32 = sugarsubstitute_contracts::application_launch::LAUNCH_POINTER_SCHEMA;
const MAX_BYTES: usize = 4096;
const MAX_ENGINE_BYTES: u64 = 1024 * 1024 * 1024;

/// One generation's launch engine, bound to its exact bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EngineTarget {
    generation: GenerationId,
    sha256: String,
    bytes: u64,
}
impl EngineTarget {
    /// Generation folder name under `releases`.
    pub fn generation(&self) -> &GenerationId {
        &self.generation
    }
    /// Hash the engine of an already verified generation folder for publication.
    pub(in crate::setup_storage) fn of(
        generation: &GenerationId,
        directory: &Directory,
        context: &JobContext,
    ) -> io::Result<Self> {
        let mut engine = directory.read(LAUNCH_ENGINE)?;
        let target = Self {
            generation: generation.clone(),
            sha256: hash(&mut engine, context)?,
            bytes: files::length(&engine, context)?,
        };
        target.validate().map_err(|_| invalid("Launch engine is empty or oversized"))?;
        Ok(target)
    }
    fn validate(&self) -> Result<(), PointerFailure> {
        if self.bytes == 0
            || self.bytes > MAX_ENGINE_BYTES
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(PointerFailure::Damaged);
        }
        Ok(())
    }
}

/// Which engine starts first and which one replaces it if it cannot take over.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPointer {
    primary: EngineTarget,
    fallback: Option<EngineTarget>,
}
#[derive(Serialize, Deserialize)]
struct Wire {
    schema_version: u32,
    primary: EngineTarget,
    #[serde(default)]
    fallback: Option<EngineTarget>,
}
impl LaunchPointer {
    /// Point at `primary`, falling back to a different generation's engine.
    pub(in crate::setup_storage) fn new(
        primary: EngineTarget,
        fallback: Option<EngineTarget>,
    ) -> Self {
        let fallback = fallback.filter(|target| target.generation != primary.generation);
        Self { primary, fallback }
    }
    /// Generations named by this pointer, primary first.
    pub fn generations(&self) -> Vec<GenerationId> {
        std::iter::once(&self.primary)
            .chain(self.fallback.as_ref())
            .map(|target| target.generation.clone())
            .collect()
    }
    /// Engine started first.
    pub fn primary(&self) -> &EngineTarget {
        &self.primary
    }
    /// Engine started when the primary is missing, mismatched, exits or stays silent.
    pub fn fallback(&self) -> Option<&EngineTarget> {
        self.fallback.as_ref()
    }
    /// Decode bounded bytes, ignoring fields added by later releases.
    /// # Errors
    /// `Incompatible` for any other schema number; `Damaged` for anything unreadable.
    pub fn decode(bytes: &[u8]) -> Result<Self, PointerFailure> {
        if bytes.len() > MAX_BYTES {
            return Err(PointerFailure::Damaged);
        }
        #[derive(Deserialize)]
        struct Version {
            schema_version: u32,
        }
        let version: Version =
            serde_json::from_slice(bytes).map_err(|_| PointerFailure::Damaged)?;
        if version.schema_version != SCHEMA {
            return Err(PointerFailure::Incompatible);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| PointerFailure::Damaged)?;
        wire.primary.validate()?;
        if let Some(fallback) = &wire.fallback {
            fallback.validate()?;
            if fallback.generation == wire.primary.generation {
                return Err(PointerFailure::Damaged);
            }
        }
        Ok(Self { primary: wire.primary, fallback: wire.fallback })
    }
    fn encode(&self) -> io::Result<Vec<u8>> {
        let bytes = serde_json::to_vec(&Wire {
            schema_version: SCHEMA,
            primary: self.primary.clone(),
            fallback: self.fallback.clone(),
        })
        .map_err(io::Error::other)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("Launch pointer exceeds its bound"));
        }
        Ok(bytes)
    }
}

/// Why the stable entry cannot use a pointer or engine.
#[derive(Debug)]
pub enum PointerFailure {
    /// No pointer exists; the installation needs setup.
    Missing,
    /// A later release made an incompatible pointer change this root cannot read.
    Incompatible,
    /// The pointer or a target is malformed, unprotected or out of bounds.
    Damaged,
    /// The named engine is missing or its bytes differ from the pointer.
    EngineMismatch,
    /// Storage failed; the original cause is preserved.
    Storage(io::Error),
}
impl std::fmt::Display for PointerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "The launch pointer is missing",
            Self::Incompatible => "The launch pointer was written in an incompatible format",
            Self::Damaged => "The launch pointer is damaged",
            Self::EngineMismatch => "The launch engine is missing or changed",
            Self::Storage(_) => "The launch pointer could not be read",
        })
    }
}
impl std::error::Error for PointerFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(cause) => Some(cause),
            _ => None,
        }
    }
}
impl From<io::Error> for PointerFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}

/// Read the protected pointer of the installation at `installation`.
/// Run on an admitted storage worker. Reads only the pointer file.
/// # Errors
/// `Missing`, `Incompatible`, `Damaged` or `Storage`.
pub fn read_launch_pointer(
    installation: &Path,
    context: &JobContext,
) -> Result<LaunchPointer, PointerFailure> {
    let bytes = protection::with(context, |protection| {
        let root = Directory::open(installation, false)?;
        let state = Directory::open(&super::ownership::state_path(&root), false)?;
        read_bytes(&state, protection)
    })?;
    LaunchPointer::decode(&bytes.ok_or(PointerFailure::Missing)?)
}

/// Generations the stable entry may start: the pointer's primary and fallback, in that order.
/// Retirement must keep both. A missing pointer names none.
/// Run on an admitted storage worker with `POINTER_RESERVATION_BYTES`.
/// # Errors
/// `Incompatible`, `Damaged` or `Storage`: a pointer that cannot be read leaves every generation
/// it might name in doubt, so callers keep all generations.
pub fn pointed_generations(
    installation: &Path,
    context: &JobContext,
) -> Result<Vec<GenerationId>, PointerFailure> {
    match read_launch_pointer(installation, context) {
        Ok(pointer) => Ok(pointer.generations()),
        Err(PointerFailure::Missing) => Ok(Vec::new()),
        Err(cause) => Err(cause),
    }
}

/// Read the pointer under protection the caller already holds.
pub(in crate::setup_storage) fn read(
    state: &Directory,
    protection: &Protection,
) -> Result<LaunchPointer, PointerFailure> {
    LaunchPointer::decode(&read_bytes(state, protection)?.ok_or(PointerFailure::Missing)?)
}

fn read_bytes(state: &Directory, protection: &Protection) -> io::Result<Option<Vec<u8>>> {
    let mut file = match state.read(LAUNCH_POINTER_FILE) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    protection.verify_file(&file)?;
    files::single_link(&file)?;
    Ok(Some(files::read_bounded(&mut file, MAX_BYTES)?))
}

/// An engine image held against writes and deletion after its bytes matched the pointer.
/// Keep it until the engine process exists.
pub struct PinnedEngine {
    _file: File,
    _pins: Directory,
    executable: PathBuf,
    directory: PathBuf,
}
impl PinnedEngine {
    /// Verified engine image path.
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    /// Generation folder the engine runs in.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// Open the target's engine denying writes and deletion, then require its exact size and hash.
/// Never returns an engine whose bytes differ from the pointer.
/// # Errors
/// `EngineMismatch` for a missing, linked, unprotected or changed engine; `Storage` otherwise.
pub fn pin_engine(
    installation: &Path,
    target: &EngineTarget,
    context: &JobContext,
) -> Result<PinnedEngine, PointerFailure> {
    check(context)?;
    let path = installation.join("releases").join(target.generation.as_str());
    let mismatch = |cause: io::Error| {
        if cause.kind() == io::ErrorKind::NotFound || cause.kind() == io::ErrorKind::InvalidData {
            PointerFailure::EngineMismatch
        } else {
            PointerFailure::Storage(cause)
        }
    };
    let pins = Directory::open(&path, false).map_err(mismatch)?;
    // A regular, non-reparse file opened sharing only reads: no writer or deleter can change it.
    let mut file = pins.read(LAUNCH_ENGINE).map_err(|_| PointerFailure::EngineMismatch)?;
    protection::with(context, |protection| protection.verify_file(&file))
        .map_err(|_| PointerFailure::EngineMismatch)?;
    files::single_link(&file).map_err(|_| PointerFailure::EngineMismatch)?;
    if files::length(&file, context)? != target.bytes || hash(&mut file, context)? != target.sha256
    {
        return Err(PointerFailure::EngineMismatch);
    }
    let executable = path.join(LAUNCH_ENGINE);
    Ok(PinnedEngine { _file: file, _pins: pins, executable, directory: path })
}

/// Replace the pointer atomically in the protected state folder.
/// The caller serializes writers (installation custody or the selection writer).
pub(in crate::setup_storage) fn publish(
    state: &Directory,
    pointer: &LaunchPointer,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let bytes = pointer.encode()?;
    let mut stage = publication::Stage::create(state, protection, context)?;
    std::io::Write::write_all(stage.file(), &bytes)?;
    files::sync(stage.file(), context)?;
    stage.publish(&state.path().join(LAUNCH_POINTER_FILE), true, protection, context)
}

fn hash(file: &mut File, context: &JobContext) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = sugarsubstitute_update::package::PayloadHasher::new();
    let mut buffer = [0; 65536];
    loop {
        check(context)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(hasher.finish_hex())
}

#[cfg(test)]
pub(crate) mod tests;
