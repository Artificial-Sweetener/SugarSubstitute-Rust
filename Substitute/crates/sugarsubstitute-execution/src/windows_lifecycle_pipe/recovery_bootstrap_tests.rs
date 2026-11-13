// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep recovery inheritance purpose distinct from every existing captured/ordinary schema.
use super::*;
use sugarsubstitute_contracts::{incident::*, recovery_supervisor::*};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn input() -> Result<Data> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let previous =
        RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    let report = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(previous)),
        IncidentRevision::new(5)?,
        ReportSessionId::new(6)?,
    );
    let key = RecoveryGrantKey::new(RecoveryGrantId::new(7)?, previous, report)?;
    let binding = PeerBinding::Recovery(key);
    Ok(Data {
        binding,
        pid: 8,
        birth: 9,
        session: 10,
        deadline: LifecyclePipeDeadline::new(11, 12)?,
        capability: [13; 32],
        name: name(binding),
    })
}
#[test]
fn recovery_bootstrap_retains_complete_run_report_grant_and_original_counter() -> Result {
    let source = input()?;
    let encoded = encode(&source)?;
    let decoded = decode(&encoded, source.deadline, Kind::Recovery)?;
    assert_eq!(encoded[4..8], [6, 0, 0, 4]);
    assert_eq!(decoded.binding, source.binding);
    assert_eq!(decoded.name, source.name);
    assert_eq!((decoded.pid, decoded.birth, decoded.session), (8, 9, 10));
    assert_eq!(decoded.deadline, source.deadline);
    assert_eq!(decoded.capability, [13; 32]);
    assert_eq!(decoded.binding.capture_target(), None);
    assert_eq!(decoded.binding.desktop(), None);
    assert_eq!(decoded.binding.backend_custody(), None);
    Ok(())
}
#[test]
fn recovery_bootstrap_cannot_cross_any_ordinary_or_captured_purpose() -> Result {
    let source = input()?;
    let encoded = encode(&source)?;
    for kind in [
        Kind::Desktop,
        Kind::BackendCustody,
        Kind::Diagnostic(DiagnosticRole::Storage),
        Kind::Diagnostic(DiagnosticRole::Symbolize),
        Kind::Diagnostic(DiagnosticRole::Reporter),
    ] {
        assert!(decode(&encoded, source.deadline, kind).is_err());
        assert!(decode_captured_base(&encoded, source.deadline, kind).is_err());
    }
    assert!(encode_captured_base(&source).is_err());
    assert!(decode_captured_base(&encoded, source.deadline, Kind::Recovery).is_err());
    let binding =
        PeerBinding::Desktop(source.binding.recovery().ok_or("Recovery binding missing")?.run());
    let existing = Data { binding, name: name(binding), ..input()? };
    assert!(decode(&encode(&existing)?, existing.deadline, Kind::Recovery).is_err());
    Ok(())
}
#[test]
fn recovery_bootstrap_rejects_reserved_changed_owner_grant_name_and_original_deadline() -> Result {
    let source = input()?;
    let valid = encode(&source)?;
    for at in [
        4,
        6,
        56,
        60,
        64,
        72,
        80,
        104,
        138,
        144,
        666,
        RECOVERY_KEY_START,
        RECOVERY_KEY_START + 8,
        RECOVERY_KEY_END,
        1023,
    ] {
        let mut changed = valid;
        if at == 104 {
            changed[104..136].fill(0);
        } else if at == 72 {
            changed[72..76].fill(0);
        } else if at == 80 {
            changed[80..88].fill(0);
        } else {
            changed[at] ^= 1;
        }
        assert!(decode(&changed, source.deadline, Kind::Recovery).is_err(), "changed byte {at}");
    }
    let changed = LifecyclePipeDeadline::new(12, 12)?;
    assert!(decode(&valid, changed, Kind::Recovery).is_err());
    assert!(decode(&valid, source.deadline, Kind::Recovery).is_ok());
    Ok(())
}
