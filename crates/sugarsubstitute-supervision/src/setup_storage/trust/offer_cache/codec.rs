// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Four bounded length-prefixed byte strings inside the versioned installation envelope.
use super::*;

/// Preserve every signed byte without JSON escaping or interpreting metadata fields.
pub(super) fn encode(metadata: MetadataSet<'_>, descriptor: &[u8]) -> io::Result<Vec<u8>> {
    let parts = [metadata.timestamp, metadata.snapshot, metadata.targets, descriptor];
    if parts.iter().enumerate().any(|(index, part)| part.is_empty() || part.len() > limit(index)) {
        return Err(invalid("Cached offer field exceeds its bound"));
    }
    let mut bytes = Vec::with_capacity(16 + parts.iter().map(|part| part.len()).sum::<usize>());
    for part in parts {
        bytes.extend_from_slice(&(part.len() as u32).to_le_bytes());
        bytes.extend_from_slice(part);
    }
    Ok(bytes)
}

/// Validate ownership format before replacing an existing cache record.
pub(super) fn validate(bytes: &[u8]) -> io::Result<()> {
    parts(bytes).map(|_| ())
}

/// The request comes exclusively from the caller; cache content supplies only signed bytes.
pub(super) fn decode(bytes: &[u8], request: Request) -> io::Result<OfferInputs> {
    let [timestamp, snapshot, targets, descriptor] = parts(bytes)?;
    Ok(OfferInputs {
        request,
        timestamp: timestamp.to_vec(),
        snapshot: snapshot.to_vec(),
        targets: targets.to_vec(),
        descriptor: descriptor.to_vec(),
    })
}

/// Enforce field bounds before allocating, including complete consumption of the record.
fn parts(mut bytes: &[u8]) -> io::Result<[&[u8]; 4]> {
    let mut parts = [&[][..]; 4];
    for (index, part) in parts.iter_mut().enumerate() {
        let length = bytes.get(..4).ok_or_else(|| invalid("Cached offer length is absent"))?;
        let length = u32::from_le_bytes(length.try_into().map_err(io::Error::other)?) as usize;
        bytes = &bytes[4..];
        if length == 0 || length > limit(index) {
            return Err(invalid("Cached offer field exceeds its bound"));
        }
        *part = bytes.get(..length).ok_or_else(|| invalid("Cached offer field is truncated"))?;
        bytes = &bytes[length..];
    }
    if !bytes.is_empty() {
        return Err(invalid("Cached offer has trailing bytes"));
    }
    Ok(parts)
}

/// Descriptor and repository roles retain their existing independent wire bounds.
fn limit(index: usize) -> usize {
    if index == 3 {
        sugarsubstitute_contracts::update::MAX_OFFER_BYTES
    } else {
        sugarsubstitute_update::MAX_METADATA_BYTES
    }
}
