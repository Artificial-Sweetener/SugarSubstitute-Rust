// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonically encode inert grant/report correlation independently of native authentication.
use super::*;
use crate::{incident::*, lifecycle::*};
/// Exact matching-key extent used by this wire and its dedicated inherited bootstrap.
pub const KEY_BYTES: usize = 160;
/// Write exact inert grant matching data into a fixed160-byte destination.
/// # Errors
/// Wrong destination length returns Capacity before any write.
pub fn encode(key: RecoveryGrantKey, out: &mut [u8]) -> Result<(), RecoveryWireError> {
    if out.len() != KEY_BYTES {
        return Err(RecoveryWireError::Capacity);
    }
    out.fill(0);
    put(out, 0, key.id().get());
    let previous = key.previous();
    out[8..40].copy_from_slice(previous.owner.namespace.bytes());
    out[40..56].copy_from_slice(previous.owner.epoch.bytes());
    put(out, 56, previous.instance.get());
    put(out, 64, previous.generation.get());
    let report = key.report();
    let subject = report.incident().subject();
    let owner = subject.coordinator();
    out[72..104].copy_from_slice(owner.namespace.bytes());
    out[104..120].copy_from_slice(owner.epoch.bytes());
    if let IncidentSubject::Desktop(run) = subject {
        out[120..124].copy_from_slice(&1u32.to_le_bytes());
        put(out, 128, run.instance.get());
        put(out, 136, run.generation.get());
    }
    put(out, 144, report.revision().get());
    put(out, 152, report.session().get());
    Ok(())
}
/// Decode canonical matching data; callers must separately authenticate the live session.
/// # Errors
/// Unknown subjects, reserved bytes, zero identities and changed namespaces reject.
pub fn decode(bytes: &[u8]) -> Result<RecoveryGrantKey, RecoveryWireError> {
    if bytes.len() != KEY_BYTES || bytes[124..128] != [0; 4] {
        return Err(RecoveryWireError::Malformed);
    }
    let owner = CoordinatorKey {
        namespace: NamespaceId::new(array(bytes, 8)?),
        epoch: CoordinatorEpoch::new(array(bytes, 40)?).map_err(|_| RecoveryWireError::Binding)?,
    };
    let previous = RunKey {
        owner,
        instance: InstanceId::new(number(bytes, 56)?).map_err(|_| RecoveryWireError::Binding)?,
        generation: RunGeneration::new(number(bytes, 64)?)
            .map_err(|_| RecoveryWireError::Binding)?,
    };
    let owner = CoordinatorKey {
        namespace: NamespaceId::new(array(bytes, 72)?),
        epoch: CoordinatorEpoch::new(array(bytes, 104)?).map_err(|_| RecoveryWireError::Binding)?,
    };
    let subject = match u32::from_le_bytes(array(bytes, 120)?) {
        0 if number(bytes, 128)? == 0 && number(bytes, 136)? == 0 => {
            IncidentSubject::Supervisor(owner)
        }
        1 => IncidentSubject::Desktop(RunKey {
            owner,
            instance: InstanceId::new(number(bytes, 128)?)
                .map_err(|_| RecoveryWireError::Binding)?,
            generation: RunGeneration::new(number(bytes, 136)?)
                .map_err(|_| RecoveryWireError::Binding)?,
        }),
        _ => return Err(RecoveryWireError::Malformed),
    };
    let report = ReportBinding::new(
        IncidentKey::new(subject),
        IncidentRevision::new(number(bytes, 144)?).map_err(|_| RecoveryWireError::Binding)?,
        ReportSessionId::new(number(bytes, 152)?).map_err(|_| RecoveryWireError::Binding)?,
    );
    RecoveryGrantKey::new(
        RecoveryGrantId::new(number(bytes, 0)?).map_err(|_| RecoveryWireError::Binding)?,
        previous,
        report,
    )
    .map_err(|_| RecoveryWireError::Binding)
}
pub(super) fn array<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], RecoveryWireError> {
    bytes.get(at..at + N).and_then(|v| v.try_into().ok()).ok_or(RecoveryWireError::Malformed)
}
pub(super) fn number(bytes: &[u8], at: usize) -> Result<u64, RecoveryWireError> {
    Ok(u64::from_le_bytes(array(bytes, at)?))
}
pub(super) fn put(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
