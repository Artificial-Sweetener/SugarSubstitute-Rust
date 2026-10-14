// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist the original native allocation in one finite central worker without UI I/O or hash work.

use sha2::{Digest as _, Sha256};
use std::{error::Error, fmt, path::PathBuf};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryClaimError, Moment};
use sugarsubstitute_desktop::CapturedMainPreview;
use sugarsubstitute_execution::{ExecutionClient, PreviewArmingError, PreviewSourceLoan};
use sugarsubstitute_supervision::{ClaimError, qualification_artifact::PreviewArtifact};

pub(super) const JOB_BYTES: u64 = 64 * 1024;
pub(super) const IMAGE_BYTES: u64 = 2 * 1024 * 1024;
const WORKER_BYTES: u64 = 16 * 1024;

/// Bound all additional authored controller/payload/hash/seal/adapter storage cumulatively.
pub(super) fn layout_fits(destination_capacity: usize) -> bool {
    if destination_capacity > 4096 {
        return false;
    }
    let worker = size_of::<super::Preview>()
        + size_of::<PersistInput>()
        + size_of::<Persisted>()
        + size_of::<PersistFailure>()
        + size_of::<Sha256>()
        + destination_capacity
        + 1024;
    let seal = size_of::<sugarsubstitute_execution::PreviewArmingSeal>() + destination_capacity;
    worker <= WORKER_BYTES as usize
        && WORKER_BYTES as usize
            + seal
            + sugarsubstitute_supervision::qualification_artifact::STORAGE_BYTES as usize
            <= JOB_BYTES as usize
}

pub(super) struct PersistInput {
    pub(super) destination: Option<PathBuf>,
    pub(super) source: Option<PreviewSourceLoan>,
    pub(super) client: ExecutionClient,
    pub(super) image: CapturedMainPreview,
    pub(super) until: Moment,
}
pub(super) struct Persisted {
    pub(super) image: CapturedMainPreview,
    pub(super) artifact: PreviewArtifact,
    pub(super) digest: [u8; 32],
    _storage: MemoryClaim,
}
pub(super) struct PersistFailure {
    cause: PersistCause,
    close: Option<ClaimError>,
    image: CapturedMainPreview,
    artifact: Option<PreviewArtifact>,
    unbound: Option<(Option<PreviewSourceLoan>, Option<PathBuf>)>,
    _storage: Option<MemoryClaim>,
}
#[derive(Debug)]
enum PersistCause {
    Filesystem(ClaimError),
    Arming(PreviewArmingError),
    Storage(MemoryClaimError),
    Stopped,
}
impl fmt::Display for PersistCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem(cause) => fmt::Display::fmt(cause, formatter),
            Self::Arming(cause) => fmt::Display::fmt(cause, formatter),
            Self::Storage(cause) => fmt::Display::fmt(cause, formatter),
            Self::Stopped => formatter.write_str("Original preview job cancelled or expired"),
        }
    }
}
impl Error for PersistCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filesystem(cause) => Some(cause),
            Self::Arming(cause) => Some(cause),
            Self::Storage(cause) => Some(cause),
            Self::Stopped => None,
        }
    }
}
impl PersistFailure {
    /// An observed worker failure does not imply all its actual native outputs closed.
    pub(super) fn retired(&self) -> bool {
        self.artifact.as_ref().is_none_or(PreviewArtifact::retired)
    }
}
impl fmt::Debug for PersistFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistFailure")
            .field("native_retired", &self.retired())
            .field("identity", &self.image.snapshot().identity())
            .field("close_failed", &self.close.is_some())
            .field("unbound_input_retained", &self.unbound.is_some())
            .finish()
    }
}
impl fmt::Display for PersistFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.cause, formatter)
    }
}
impl Error for PersistFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Own all original bytes and native failure custody until the actual worker terminal is consumed.
#[expect(
    clippy::result_large_err,
    reason = "Return original funded image/source/native failure custody inline; rejection cannot allocate a new fallible box"
)]
pub(super) fn persist(
    input: PersistInput,
    context: JobContext,
) -> Result<Persisted, PersistFailure> {
    let PersistInput { mut destination, mut source, client, image, until } = input;
    let storage = match context.memory().claim(WORKER_BYTES) {
        Ok(storage) => storage,
        Err(cause) => {
            return Err(PersistFailure {
                cause: PersistCause::Storage(cause),
                close: None,
                image,
                artifact: None,
                unbound: Some((source, destination)),
                _storage: None,
            });
        }
    };
    let snapshot = image.snapshot();
    let viewport = snapshot.viewport();
    let seal = match client.bind_preview_source(
        &context,
        &mut source,
        &mut destination,
        snapshot.identity(),
    ) {
        Ok(seal) => seal,
        Err(cause) => {
            return Err(PersistFailure {
                cause: PersistCause::Arming(cause),
                close: None,
                image,
                artifact: None,
                unbound: Some((source, destination)),
                _storage: Some(storage),
            });
        }
    };
    let created = PreviewArtifact::new(
        seal,
        snapshot.identity(),
        (viewport.width(), viewport.height()),
        &context,
    );
    let mut artifact = match created {
        Ok(artifact) => artifact,
        Err(cause) => {
            return Err(PersistFailure {
                cause: PersistCause::Filesystem(cause),
                close: None,
                image,
                artifact: None,
                unbound: None,
                _storage: Some(storage),
            });
        }
    };
    let result = (|| {
        artifact.initialize().map_err(PersistCause::Filesystem)?;
        let mut hash = Sha256::new();
        for chunk in image.as_bytes().chunks(16 * 1024) {
            if context.cancellation_requested() || context.now() >= until {
                return Err(PersistCause::Stopped);
            }
            hash.update(chunk);
        }
        let digest: [u8; 32] = hash.finalize().into();
        artifact.write_png(image.as_bytes()).map_err(PersistCause::Filesystem)?;
        artifact.activate().map_err(PersistCause::Filesystem)?;
        Ok(digest)
    })();
    let close = artifact.retire();
    match (result, close) {
        (Ok(digest), Ok(())) => Ok(Persisted { image, artifact, digest, _storage: storage }),
        (Err(cause), closed) => Err(PersistFailure {
            cause,
            close: closed.err(),
            image,
            artifact: Some(artifact),
            unbound: None,
            _storage: Some(storage),
        }),
        (Ok(_), Err(cause)) => Err(PersistFailure {
            cause: PersistCause::Filesystem(cause),
            close: None,
            image,
            artifact: Some(artifact),
            unbound: None,
            _storage: Some(storage),
        }),
    }
}

/// Format only a fixed public artifact digest without allocating strings or exposing paths.
pub(super) struct Digest<'a>(pub(super) &'a [u8; 32]);
impl fmt::Display for Digest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
