// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode only bounded matching facts in the sole captured startup section.
use super::super::native::bootstrap;
use crate::{
    capture_attach::types::CaptureEndpointObservation,
    lifecycle_pipe::{LifecyclePipeDeadline, LifecyclePipeFailure, failure},
};
use std::ops::Range;
use sugarsubstitute_contracts::execution::CounterDeadline;

pub(in crate::windows_lifecycle_pipe) const BASE_BYTES: usize = 2048;
pub(in crate::windows_lifecycle_pipe) const PREFIX_BYTES: usize = 16;
pub(in crate::windows_lifecycle_pipe) const MAX_PAYLOAD_BYTES: usize = 65416;
pub(in crate::windows_lifecycle_pipe) const MAX_LOGICAL_BYTES: usize =
    BASE_BYTES + PREFIX_BYTES + MAX_PAYLOAD_BYTES;

/// Inert decoded matching facts; no constructor grants process/map or release authority.
/// Actual kernel self/creator/handler and lifecycle peer validation happens separately.
pub(crate) struct CapturedFacts {
    pub(crate) creator_pid: u32,
    pub(crate) handler_pid: u32,
    pub(crate) creator_birth: u64,
    pub(crate) handler_birth: u64,
    pub(crate) processes: [usize; 2],
    pub(crate) source: CounterDeadline,
    pub(crate) build: [u8; 64],
    pub(crate) build_length: usize,
    pub(crate) endpoint: [u16; 256],
    pub(crate) endpoint_length: usize,
    pub(crate) payload: Range<usize>,
}
/// Write directly into the actual mapped section; original startup bytes remain owned.
/// All fields come from retained original owners, but encoding itself grants no trust.
pub(in crate::windows_lifecycle_pipe) fn encode(
    destination: &mut [u8],
    data: &bootstrap::Data,
    endpoint: CaptureEndpointObservation,
    processes: [usize; 2],
    source: CounterDeadline,
    body: &[u8],
) -> Result<(), LifecyclePipeFailure> {
    let expected = endpoint.expectation();
    if data.binding.capture_target() != Some(expected.target())
        || data.pid != expected.creator().process_id()
        || data.birth != expected.creator().creation_time_100ns()
        || (matches!(data.binding, crate::lifecycle_pipe::PeerBinding::Desktop(_))
            && body.is_empty())
        || (matches!(data.binding, crate::lifecycle_pipe::PeerBinding::BackendCustody(_))
            && !body.is_empty())
        || body.len() > MAX_PAYLOAD_BYTES
        || destination.len() != BASE_BYTES + PREFIX_BYTES + body.len()
        || !valid_handles(processes)
    {
        return Err(failure("captured_bootstrap_parent_binding"));
    }
    destination.fill(0);
    destination[..1024].copy_from_slice(&bootstrap::encode_captured_base(data)?);
    destination[1024..1028].copy_from_slice(b"SSCG");
    destination[1028..1030].copy_from_slice(&1u16.to_le_bytes());
    destination[1030..1032].copy_from_slice(&672u16.to_le_bytes());
    put32(destination, 1032, if expected.target().backend_custody().is_some() { 2 } else { 1 });
    put32(destination, 1036, 1);
    put32(destination, 1040, expected.creator().process_id());
    put32(destination, 1044, expected.handler().process_id());
    put64(destination, 1048, expected.creator().creation_time_100ns());
    put64(destination, 1056, expected.handler().creation_time_100ns());
    put64(destination, 1064, processes[0] as u64);
    put64(destination, 1072, processes[1] as u64);
    put64(destination, 1080, source.ticks() as u64);
    put64(destination, 1088, source.frequency() as u64);
    let build = expected.build_bytes();
    let name = endpoint.endpoint_utf16();
    put32(destination, 1112, build.len() as u32);
    put32(destination, 1116, name.len() as u32);
    destination[1120..1120 + build.len()].copy_from_slice(build);
    for (index, unit) in name.iter().enumerate() {
        destination[1184 + index * 2..1186 + index * 2].copy_from_slice(&unit.to_le_bytes());
    }
    destination[BASE_BYTES..BASE_BYTES + 4].copy_from_slice(b"SSCS");
    destination[BASE_BYTES + 4..BASE_BYTES + 6].copy_from_slice(&1u16.to_le_bytes());
    destination[BASE_BYTES + 6..BASE_BYTES + 8]
        .copy_from_slice(&(PREFIX_BYTES as u16).to_le_bytes());
    put32(destination, BASE_BYTES + 8, body.len() as u32);
    destination[BASE_BYTES + PREFIX_BYTES..].copy_from_slice(body);
    Ok(())
}
/// Read the bounded fixed prefix before remapping the exact serialized extent.
/// A page-rounded mapping length never substitutes for this strict payload length.
pub(in crate::windows_lifecycle_pipe) fn logical_bytes(
    bytes: &[u8],
) -> Result<usize, LifecyclePipeFailure> {
    if bytes.len() < BASE_BYTES + PREFIX_BYTES
        || &bytes[BASE_BYTES..BASE_BYTES + 4] != b"SSCS"
        || read::<2>(bytes, BASE_BYTES + 4)? != 1u16.to_le_bytes()
        || read::<2>(bytes, BASE_BYTES + 6)? != (PREFIX_BYTES as u16).to_le_bytes()
        || read::<4>(bytes, BASE_BYTES + 12)? != [0; 4]
    {
        return Err(failure("captured_startup_prefix"));
    }
    let length = u32::from_le_bytes(read(bytes, BASE_BYTES + 8)?) as usize;
    if length > MAX_PAYLOAD_BYTES {
        return Err(failure("captured_startup_bound"));
    }
    Ok(BASE_BYTES + PREFIX_BYTES + length)
}
/// Validate the full serialized envelope before actual native authentication.
/// Original successful v1/v2 codecs remain separately strict and unchanged.
pub(in crate::windows_lifecycle_pipe) fn decode(
    bytes: &[u8],
    original: LifecyclePipeDeadline,
    kind: bootstrap::Kind,
) -> Result<(bootstrap::Data, CapturedFacts), LifecyclePipeFailure> {
    let logical = logical_bytes(bytes)?;
    if logical != bytes.len() {
        return Err(failure("captured_startup_extent"));
    }
    let base: &[u8; 1024] =
        bytes[..1024].try_into().map_err(|_| failure("captured_bootstrap_base"))?;
    let data = bootstrap::decode_captured_base(base, original, kind)?;
    if &bytes[1024..1028] != b"SSCG"
        || read::<2>(bytes, 1028)? != 1u16.to_le_bytes()
        || read::<2>(bytes, 1030)? != 672u16.to_le_bytes()
        || u32::from_le_bytes(read(bytes, 1032)?)
            != if matches!(data.binding, crate::lifecycle_pipe::PeerBinding::BackendCustody(_)) {
                2
            } else {
                1
            }
        || u32::from_le_bytes(read(bytes, 1036)?) != 1
        || bytes[1096..1112].iter().chain(bytes[1696..2048].iter()).any(|value| *value != 0)
    {
        return Err(failure("captured_bootstrap_schema"));
    }
    if (matches!(data.binding, crate::lifecycle_pipe::PeerBinding::Desktop(_))
        && logical == BASE_BYTES + PREFIX_BYTES)
        || (matches!(data.binding, crate::lifecycle_pipe::PeerBinding::BackendCustody(_))
            && logical != BASE_BYTES + PREFIX_BYTES)
    {
        return Err(failure("captured_startup_purpose_payload"));
    }
    let creator_pid = u32::from_le_bytes(read(bytes, 1040)?);
    let handler_pid = u32::from_le_bytes(read(bytes, 1044)?);
    let creator_birth = u64::from_le_bytes(read(bytes, 1048)?);
    let handler_birth = u64::from_le_bytes(read(bytes, 1056)?);
    let processes = [
        u64::from_le_bytes(read(bytes, 1064)?) as usize,
        u64::from_le_bytes(read(bytes, 1072)?) as usize,
    ];
    let source = CounterDeadline::new(
        i64::from_le_bytes(read(bytes, 1080)?),
        i64::from_le_bytes(read(bytes, 1088)?),
    )
    .map_err(|cause| {
        LifecyclePipeFailure::native(crate::native::NativeObservationError::new(
            "captured_original_counter",
            cause,
        ))
    })?;
    let build_length = u32::from_le_bytes(read(bytes, 1112)?) as usize;
    let endpoint_length = u32::from_le_bytes(read(bytes, 1116)?) as usize;
    if creator_pid != data.pid
        || creator_birth != data.birth
        || handler_pid == 0
        || handler_birth == 0
        || creator_pid == handler_pid
        || !valid_handles(processes)
        || !(1..=64).contains(&build_length)
        || !(10..=256).contains(&endpoint_length)
    {
        return Err(failure("captured_bootstrap_matching_facts"));
    }
    let build = read::<64>(bytes, 1120)?;
    if build[build_length..].iter().any(|value| *value != 0)
        || !build[..build_length]
            .iter()
            .all(|value| value.is_ascii_alphanumeric() || b"_-".contains(value))
    {
        return Err(failure("captured_bootstrap_build"));
    }
    let mut endpoint = [0; 256];
    for (index, unit) in endpoint.iter_mut().enumerate() {
        *unit = u16::from_le_bytes(read(bytes, 1184 + index * 2)?);
    }
    if endpoint[endpoint_length..].iter().any(|value| *value != 0)
        || !crate::capture_attach::types::valid_endpoint(&endpoint[..endpoint_length])
    {
        return Err(failure("captured_bootstrap_endpoint"));
    }
    Ok((
        data,
        CapturedFacts {
            creator_pid,
            handler_pid,
            creator_birth,
            handler_birth,
            processes,
            source,
            build,
            build_length,
            endpoint,
            endpoint_length,
            payload: BASE_BYTES + PREFIX_BYTES..logical,
        },
    ))
}
/// Pointer-sized matching values must describe distinct nonpseudo inherited slots.
fn valid_handles(values: [usize; 2]) -> bool {
    values[0] != values[1] && values.iter().all(|value| *value != 0 && *value < usize::MAX - 15)
}
/// Access only a bounded initialized fixed field; malformed data preserves typed failure.
fn read<const N: usize>(bytes: &[u8], start: usize) -> Result<[u8; N], LifecyclePipeFailure> {
    bytes
        .get(start..start + N)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| failure("captured_bootstrap_field"))
}
/// Fixed offsets are reviewed against the bounded initialized destination envelope.
fn put32(bytes: &mut [u8], start: usize, value: u32) {
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
}
/// Fixed offsets are reviewed against the bounded initialized destination envelope.
fn put64(bytes: &mut [u8], start: usize, value: u64) {
    bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
}
