// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer exact retained bytes into independently owned encoded image-cache custody.
use super::*;
use std::{io::Seek, sync::Mutex};
use sugarsubstitute_comfy::output_media::{
    ComfyOutputFormat, ComfyOutputTransfer, retain_original,
};
use sugarsubstitute_supervision::{RetainedArtifactReader, cache::utc_seconds};

pub(super) enum Original {
    Unavailable,
    Backend(ComfyOutputTransfer),
    Retained {
        path: PathBuf,
        expected: OutputArtifactIdentity,
        format: ComfyOutputFormat,
        reader: Mutex<Option<RetainedArtifactReader>>,
    },
}
impl Original {
    pub(super) fn new(spec: OriginalSpec) -> io::Result<Self> {
        match spec {
            OriginalSpec::Unavailable => Ok(Self::Unavailable),
            OriginalSpec::Backend { backend, target, expected } => Ok(Self::Backend(
                match expected {
                    Some(expected) => ComfyOutputTransfer::verified(
                        backend.origin,
                        target,
                        backend.peer,
                        sugarsubstitute_image_io::MAX_ENCODED_SOURCE_BYTES,
                        expected,
                    ),
                    None => ComfyOutputTransfer::new(
                        backend.origin,
                        target,
                        backend.peer,
                        sugarsubstitute_image_io::MAX_ENCODED_SOURCE_BYTES,
                    ),
                }
                .map_err(io::Error::other)?,
            )),
            OriginalSpec::Retained { path, asset } => {
                let format =
                    ComfyOutputFormat::from_filename(&format!("original.{}", asset.extension))
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Retained original is not supported by the native decoder",
                            )
                        })?;
                Ok(Self::Retained {
                    path,
                    expected: OutputArtifactIdentity::new(&asset.digest, asset.bytes)
                        .map_err(io::Error::other)?,
                    format,
                    reader: Mutex::new(None),
                })
            }
        }
    }
    pub(super) fn fetch(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<ImageSource> {
        let reader = match self {
            Self::Unavailable => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Recorded original is unavailable for this native binding",
                ));
            }
            Self::Backend(transfer) => {
                transfer.fetch(cache, utc_seconds()?, context).map_err(io::Error::other)?
            }
            Self::Retained { path, expected, format, reader } => {
                let mut reader = reader.try_lock().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Retained original is already borrowed",
                    )
                })?;
                if reader.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Original acquisition already started",
                    ));
                }
                *reader =
                    Some(RetainedArtifactReader::open(path, expected.bytes(), resident, context)?);
                let original = reader
                    .as_mut()
                    .ok_or_else(|| io::Error::other("Retained native original disappeared"))?;
                let result = original
                    .with_reader(context, |input| {
                        input.rewind()?;
                        retain_original(input, *format, *expected, cache, utc_seconds()?, context)
                            .map_err(io::Error::other)
                    })
                    .map_err(io::Error::other);
                let retired = original.close(context);
                if original.closed() {
                    *reader = None;
                }
                match (result, retired) {
                    (Ok(value), Ok(())) => {
                        let digest: String =
                            expected.digest().iter().map(|byte| format!("{byte:02x}")).collect();
                        tracing::debug!(operation="workspace_native_original_verified",job_id=context.spec().id.get(),digest=%digest,bytes=expected.bytes(),"Retained original verified into independent native image-cache custody");
                        value
                    }
                    (Err(cause), Ok(())) | (Ok(_), Err(cause)) => return Err(cause),
                    (Err(original), Err(retirement)) => {
                        return Err(io::Error::other(FailedRetirement { original, retirement }));
                    }
                }
            }
        };
        ImageSource::encoded(&mut Some(reader), resident).map_err(io::Error::other)
    }
    pub(super) fn retire(&self, context: &JobContext) -> io::Result<()> {
        match self {
            Self::Unavailable => Ok(()),
            Self::Backend(transfer) => transfer.retire(context).map_err(io::Error::other),
            Self::Retained { reader, .. } => {
                let mut reader = reader
                    .lock()
                    .map_err(|_| io::Error::other("Retained original custody failed"))?;
                if let Some(original) = &mut *reader {
                    original.close(context)?;
                }
                *reader = None;
                Ok(())
            }
        }
    }
    pub(super) fn settled(&self) -> bool {
        match self {
            Self::Unavailable => true,
            Self::Backend(transfer) => transfer.settled(),
            Self::Retained { reader, .. } => reader.try_lock().is_ok_and(|reader| reader.is_none()),
        }
    }
}
#[derive(Debug)]
struct FailedRetirement {
    original: io::Error,
    retirement: io::Error,
}
impl std::fmt::Display for FailedRetirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Retained original acquisition and retirement failed: {}",
            self.retirement
        )
    }
}
impl std::error::Error for FailedRetirement {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.original)
    }
}
