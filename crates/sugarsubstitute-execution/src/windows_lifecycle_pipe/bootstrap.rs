// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish one versioned immutable unnamed section without secret argv/environment material.
use super::*;
use sugarsubstitute_contracts::incident::{DiagnosticKey, DiagnosticRole, DiagnosticSessionId};
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration,
};
pub(crate) const BOOTSTRAP_BYTES: usize = 1024;
#[path = "bootstrap_section.rs"]
mod section;
pub(crate) use section::Section;
pub(crate) struct Data {
    pub(crate) binding: PeerBinding,
    pub(crate) pid: u32,
    pub(crate) birth: u64,
    pub(crate) session: u32,
    pub(crate) deadline: LifecyclePipeDeadline,
    pub(crate) capability: [u8; 32],
    pub(crate) name: Vec<u16>,
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| [DIGITS[(b >> 4) as usize] as char, DIGITS[(b & 15) as usize] as char])
        .collect()
}
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Desktop,
    Diagnostic(DiagnosticRole),
}
fn role_code(role: DiagnosticRole) -> u32 {
    match role {
        DiagnosticRole::Storage => 1,
        DiagnosticRole::Symbolize => 2,
        DiagnosticRole::Reporter => 3,
    }
}
pub(crate) fn name(binding: PeerBinding) -> Vec<u16> {
    let value = match binding {
        PeerBinding::Desktop(run) => format!(
            "\\\\.\\pipe\\SugarSubstitute.lifecycle.v1.{}.{}.{}.{}",
            hex(run.owner.namespace.bytes()),
            hex(run.owner.epoch.bytes()),
            run.instance.get(),
            run.generation.get()
        ),
        PeerBinding::Diagnostic(key) => format!(
            "\\\\.\\pipe\\SugarSubstitute.diagnostic.v2.{}.{}.{}.{}",
            hex(key.owner().namespace.bytes()),
            hex(key.owner().epoch.bytes()),
            role_code(key.role()),
            key.session().get()
        ),
    };
    value.encode_utf16().chain(Some(0)).collect()
}
/// Preserve the supported ordinary v1/v2 header and payload bytes.
pub(crate) fn encode(data: &Data) -> Result<[u8; BOOTSTRAP_BYTES], LifecyclePipeFailure> {
    let version = if matches!(data.binding, PeerBinding::Desktop(_)) { 1 } else { 2 };
    encode_base(data, version, BOOTSTRAP_BYTES as u16)
}
/// Encode only the captured Desktop base using the same original payload owner.
pub(in crate::windows_lifecycle_pipe) fn encode_captured_base(
    data: &Data,
) -> Result<[u8; BOOTSTRAP_BYTES], LifecyclePipeFailure> {
    if !matches!(data.binding, PeerBinding::Desktop(_)) || data.capability == [0; 32] {
        return Err(failure("captured_bootstrap_kind"));
    }
    encode_base(data, 3, 2048)
}
/// Encode common fields once; header selection remains with supported entry points.
fn encode_base(
    data: &Data,
    version: u16,
    header: u16,
) -> Result<[u8; BOOTSTRAP_BYTES], LifecyclePipeFailure> {
    if matches!(data.binding, PeerBinding::Diagnostic(_)) && data.capability == [0; 32] {
        return Err(failure("pipe_bootstrap_capability"));
    }
    if data.name.len() > 256 || data.name.last() != Some(&0) {
        return Err(failure("pipe_bootstrap_name_bound"));
    }
    let mut out = [0; BOOTSTRAP_BYTES];
    out[..4].copy_from_slice(b"SSLB");
    out[4..6].copy_from_slice(&version.to_le_bytes());
    out[6..8].copy_from_slice(&header.to_le_bytes());
    out[8..40].copy_from_slice(data.binding.owner().namespace.bytes());
    out[40..56].copy_from_slice(data.binding.owner().epoch.bytes());
    match data.binding {
        PeerBinding::Desktop(run) => {
            out[56..64].copy_from_slice(&run.instance.get().to_le_bytes());
            out[64..72].copy_from_slice(&run.generation.get().to_le_bytes());
        }
        PeerBinding::Diagnostic(key) => {
            out[56..60].copy_from_slice(&2u32.to_le_bytes());
            out[60..64].copy_from_slice(&role_code(key.role()).to_le_bytes());
            out[64..72].copy_from_slice(&key.session().get().to_le_bytes());
        }
    }
    out[72..76].copy_from_slice(&data.pid.to_le_bytes());
    out[76..80].copy_from_slice(&data.session.to_le_bytes());
    out[80..88].copy_from_slice(&data.birth.to_le_bytes());
    out[88..96].copy_from_slice(&data.deadline.ticks.to_le_bytes());
    out[96..104].copy_from_slice(&data.deadline.frequency.to_le_bytes());
    out[104..136].copy_from_slice(&data.capability);
    out[136..138].copy_from_slice(&(data.name.len() as u16).to_le_bytes());
    for (index, unit) in data.name.iter().enumerate() {
        out[144 + index * 2..146 + index * 2].copy_from_slice(&unit.to_le_bytes());
    }
    Ok(out)
}
fn number<const N: usize>(bytes: &[u8], start: usize) -> Result<[u8; N], LifecyclePipeFailure> {
    bytes
        .get(start..start + N)
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| failure("pipe_bootstrap_field"))
}
/// Decode the exact supported ordinary schema through the sole base owner.
pub(crate) fn decode(
    bytes: &[u8; BOOTSTRAP_BYTES],
    expected: LifecyclePipeDeadline,
    kind: Kind,
) -> Result<Data, LifecyclePipeFailure> {
    let version = if matches!(kind, Kind::Desktop) { 1 } else { 2 };
    decode_base(bytes, expected, kind, version, BOOTSTRAP_BYTES as u16)
}
/// Decode captured Desktop common fields without granting native authentication.
pub(in crate::windows_lifecycle_pipe) fn decode_captured_base(
    bytes: &[u8; BOOTSTRAP_BYTES],
    expected: LifecyclePipeDeadline,
) -> Result<Data, LifecyclePipeFailure> {
    let data = decode_base(bytes, expected, Kind::Desktop, 3, 2048)?;
    if data.capability == [0; 32] {
        return Err(failure("captured_bootstrap_capability"));
    }
    Ok(data)
}
/// Reject common header, range, process and endpoint errors before native work.
fn decode_base(
    bytes: &[u8; BOOTSTRAP_BYTES],
    expected: LifecyclePipeDeadline,
    kind: Kind,
    version: u16,
    header: u16,
) -> Result<Data, LifecyclePipeFailure> {
    if &bytes[..4] != b"SSLB"
        || number::<2>(bytes, 4)? != version.to_le_bytes()
        || number::<2>(bytes, 6)? != header.to_le_bytes()
        || bytes[138..144].iter().any(|b| *b != 0)
    {
        return Err(failure("pipe_bootstrap_version"));
    }
    let owner = CoordinatorKey {
        namespace: NamespaceId::new(number(bytes, 8)?),
        epoch: CoordinatorEpoch::new(number(bytes, 40)?)
            .map_err(|e| error("pipe_bootstrap_epoch", e))?,
    };
    let binding = match kind {
        Kind::Desktop => PeerBinding::Desktop(RunKey {
            owner,
            instance: InstanceId::new(u64::from_le_bytes(number(bytes, 56)?))
                .map_err(|e| error("pipe_bootstrap_instance", e))?,
            generation: RunGeneration::new(u64::from_le_bytes(number(bytes, 64)?))
                .map_err(|e| error("pipe_bootstrap_generation", e))?,
        }),
        Kind::Diagnostic(role) => {
            if number::<4>(bytes, 56)? != 2u32.to_le_bytes()
                || number::<4>(bytes, 60)? != role_code(role).to_le_bytes()
            {
                return Err(failure("pipe_bootstrap_diagnostic_role"));
            }
            let session = DiagnosticSessionId::new(u64::from_le_bytes(number(bytes, 64)?))
                .map_err(|e| error("pipe_bootstrap_diagnostic_session", e))?;
            PeerBinding::Diagnostic(DiagnosticKey::new(owner, role, session))
        }
    };
    let deadline = LifecyclePipeDeadline::new(
        i64::from_le_bytes(number(bytes, 88)?),
        i64::from_le_bytes(number(bytes, 96)?),
    )
    .map_err(|e| error("pipe_bootstrap_deadline", e))?;
    let count = u16::from_le_bytes(number(bytes, 136)?) as usize;
    if deadline != expected
        || count == 0
        || count > 256
        || bytes[144 + count * 2..].iter().any(|b| *b != 0)
    {
        return Err(failure("pipe_bootstrap_bounds"));
    }
    let name: Vec<u16> = bytes[144..144 + count * 2]
        .chunks_exact(2)
        .map(|u| u16::from_le_bytes([u[0], u[1]]))
        .collect();
    if name != self::name(binding) {
        return Err(failure("pipe_bootstrap_endpoint"));
    }
    let pid = u32::from_le_bytes(number(bytes, 72)?);
    let birth = u64::from_le_bytes(number(bytes, 80)?);
    if pid == 0 || birth == 0 {
        return Err(failure("pipe_bootstrap_process"));
    }
    let capability = number(bytes, 104)?;
    if matches!(kind, Kind::Diagnostic(_)) && capability == [0; 32] {
        return Err(failure("pipe_bootstrap_capability"));
    }
    Ok(Data {
        binding,
        pid,
        birth,
        session: u32::from_le_bytes(number(bytes, 76)?),
        deadline,
        capability,
        name,
    })
}
#[cfg(test)]
#[path = "diagnostic_bootstrap_tests.rs"]
mod diagnostic_tests;
#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
