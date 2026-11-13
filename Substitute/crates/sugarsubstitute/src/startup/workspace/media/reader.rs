// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify retained bytes once on their immutable native handle, then read bounded fresh ranges.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};
use sugarsubstitute_comfy::output_media::{
    ComfyOutputFormat, OutputArtifactIdentity, probe_output_animation,
};
use sugarsubstitute_desktop::{MAX_MEDIA_BYTES, SessionMediaRange};

/// Compare complete verified artifact facts without opening or hashing a path.
pub(super) fn prepared(
    registry: &Mutex<BTreeMap<String, Verified>>,
    asset: &SessionAsset,
) -> Result<bool, StoreError> {
    let registry = registry.try_lock().map_err(|_| StoreError::Changed)?;
    Ok(registry.get(&asset.digest).is_some_and(|reader| {
        reader.verified && reader.asset == *asset && reader.reader.len() == asset.bytes
    }))
}
/// Acquire and verify immutable native custody under the original load/acquisition job.
/// Failure leaves any opened handle with its registry owner for checked retirement.
pub(super) fn prepare(
    registry: &Mutex<BTreeMap<String, Verified>>,
    path: &Path,
    asset: &SessionAsset,
    resident: &MemoryLease,
    context: &JobContext,
) -> Result<(), StoreError> {
    let expected =
        OutputArtifactIdentity::new(&asset.digest, asset.bytes).map_err(io::Error::other)?;
    let format =
        ComfyOutputFormat::from_artifact_filename(&format!("retained.{}", asset.extension))
            .ok_or(StoreError::Changed)?;
    let mut registry = registry.try_lock().map_err(|_| {
        StoreError::Storage(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Retained artifact is already borrowed",
        ))
    })?;
    if !registry.contains_key(&asset.digest) {
        if registry.len() >= 64 {
            let oldest = registry
                .iter()
                .filter(|(_, reader)| {
                    !reader.verified || reader.reader.len() <= LARGE_ORIGINAL_BYTES
                })
                .min_by_key(|(_, reader)| reader.touched)
                .map(|(digest, _)| digest.clone())
                .ok_or(StoreError::Changed)?;
            registry.get_mut(&oldest).ok_or(StoreError::Changed)?.reader.close(context)?;
            registry.remove(&oldest);
        }
        let reader = RetainedArtifactReader::open(path, asset.bytes, resident, context)?;
        registry.insert(
            asset.digest.clone(),
            Verified {
                reader,
                asset: asset.clone(),
                animated: false,
                verified: false,
                touched: context.now(),
            },
        );
    }
    let verified = registry.get_mut(&asset.digest).ok_or(StoreError::Changed)?;
    verified.touched = context.now();
    if verified.asset != *asset || verified.reader.len() != asset.bytes {
        return Err(StoreError::Changed);
    }
    if !verified.verified {
        tracing::debug!(operation="workspace_original_verifying",job_id=context.spec().id.get(),digest=%asset.digest,bytes=asset.bytes,"Verifying immutable retained original within acquisition admission");
        let animated = verified
            .reader
            .with_reader(context, |input| {
                input.seek(SeekFrom::Start(0))?;
                let _scratch = context.memory().claim(64 * 1024).map_err(io::Error::other)?;
                let mut bytes = vec![0; 64 * 1024];
                let mut hash = Sha256::new();
                let mut prefix = [0u8; 12];
                let mut prefix_len = 0;
                loop {
                    let count = input.read(&mut bytes)?;
                    if count == 0 {
                        break;
                    }
                    if prefix_len < prefix.len() {
                        let copied = count.min(prefix.len() - prefix_len);
                        prefix[prefix_len..prefix_len + copied].copy_from_slice(&bytes[..copied]);
                        prefix_len += copied;
                    }
                    hash.update(&bytes[..count]);
                }
                if !format.matches_prefix(&prefix[..prefix_len])
                    || <[u8; 32]>::from(hash.finalize()) != *expected.digest()
                {
                    return Err(StoreError::Changed);
                }
                probe_output_animation(input, format, || controls(context))
                    .map_err(|cause| StoreError::Storage(io::Error::other(cause)))
            })
            .map_err(|cause| StoreError::Storage(io::Error::other(cause)))?;
        verified.animated = animated;
        verified.verified = true;
        tracing::debug!(operation="workspace_original_verified",job_id=context.spec().id.get(),digest=%asset.digest,bytes=asset.bytes,"Immutable original verified and pinned for later requests");
    }
    Ok(())
}
/// Borrow only a previously verified handle under this request's fresh original controls.
/// This path performs neither first verification nor filesystem name resolution.
pub(super) fn read(
    registry: &Mutex<BTreeMap<String, Verified>>,
    asset: &SessionAsset,
    range: Option<SessionMediaRange>,
    head: bool,
    context: &JobContext,
) -> Result<MediaRead, StoreError> {
    let format =
        ComfyOutputFormat::from_artifact_filename(&format!("retained.{}", asset.extension))
            .ok_or(StoreError::Changed)?;
    let mut registry = registry.try_lock().map_err(|_| StoreError::Changed)?;
    let verified = registry.get_mut(&asset.digest).ok_or(StoreError::Changed)?;
    if !verified.verified || verified.asset != *asset || verified.reader.len() != asset.bytes {
        return Err(StoreError::Changed);
    }
    verified.touched = context.now();
    let mime = format.content_type();
    if head {
        return Ok(MediaRead {
            mime,
            total: asset.bytes,
            start: None,
            bytes: vec![],
            memory: None,
            animated: verified.animated,
            head: true,
        });
    }
    let (start, length) = extent(range, asset.bytes)?;
    let memory = context.memory().claim(length).map_err(io::Error::other)?;
    let bytes = verified
        .reader
        .with_reader(context, |input| {
            input.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length as usize).map_err(io::Error::other)?;
            bytes.resize(length as usize, 0);
            for chunk in bytes.chunks_mut(64 * 1024) {
                controls(context)?;
                input.read_exact(chunk)?;
            }
            Ok::<_, io::Error>(bytes)
        })
        .map_err(|cause| StoreError::Storage(io::Error::other(cause)))?;
    Ok(MediaRead {
        mime,
        total: asset.bytes,
        start: range.map(|_| start),
        bytes,
        memory: Some(memory),
        animated: verified.animated,
        head: false,
    })
}
fn extent(range: Option<SessionMediaRange>, total: u64) -> Result<(u64, u64), StoreError> {
    let maximum = MAX_MEDIA_BYTES as u64;
    let (start, end) = match range {
        Some(SessionMediaRange::From { start, end }) if start < total => {
            (start, end.unwrap_or(total - 1).min(total - 1).min(start.saturating_add(maximum - 1)))
        }
        Some(SessionMediaRange::Suffix { bytes }) if bytes > 0 => {
            let bytes = bytes.min(total).min(maximum);
            (total - bytes, total - 1)
        }
        None if total <= maximum && total > 0 => (0, total - 1),
        _ => return Err(StoreError::Protected),
    };
    if end < start {
        return Err(StoreError::Changed);
    }
    Ok((start, end - start + 1))
}
pub(super) fn controls(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if context.deadline().is_none_or(|cutoff| context.now() >= cutoff) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
