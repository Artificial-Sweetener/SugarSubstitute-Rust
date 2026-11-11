// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed immutable bootstrap bytes before any client connection effect.
use super::*;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn data() -> TestResult<Data> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    };
    Ok(Data {
        binding: PeerBinding::Desktop(run),
        pid: 71,
        birth: 99,
        session: 1,
        deadline: LifecyclePipeDeadline::new(1000, 100)?,
        capability: [7; 32],
        name: name(PeerBinding::Desktop(run)),
    })
}
#[test]
fn bootstrap_roundtrip_preserves_exact_scoped_identity_capability_and_original_deadline()
-> TestResult {
    let data = data()?;
    let encoded = encode(&data)?;
    let decoded = decode(&encoded, data.deadline, Kind::Desktop)?;
    assert_eq!(decoded.binding.desktop(), data.binding.desktop());
    assert_eq!((decoded.pid, decoded.birth, decoded.session), (71, 99, 1));
    assert_eq!(decoded.capability, [7; 32]);
    assert_eq!(decoded.name, data.name);
    assert_eq!(decoded.deadline, data.deadline);
    assert_eq!(encoded.len(), 1024);
    Ok(())
}
#[test]
fn bootstrap_version_scope_process_endpoint_padding_and_deadline_fail_closed() -> TestResult {
    let data = data()?;
    let valid = encode(&data)?;
    for index in [0, 4, 6, 40, 56, 64, 72, 80, 88, 96, 138, 144, 1023] {
        let mut bytes = valid;
        if [40, 56, 64, 72, 80].contains(&index) {
            let count = if index == 40 {
                16
            } else if index == 72 {
                4
            } else {
                8
            };
            bytes[index..index + count].fill(0);
        } else {
            bytes[index] ^= 1;
        }
        assert!(
            decode(&bytes, data.deadline, Kind::Desktop).is_err(),
            "accepted altered offset {index}"
        );
    }
    let mut oversized = valid;
    oversized[136..138].copy_from_slice(&257u16.to_le_bytes());
    assert!(decode(&oversized, data.deadline, Kind::Desktop).is_err());
    assert!(decode(&valid, LifecyclePipeDeadline::new(1001, 100)?, Kind::Desktop).is_err());
    Ok(())
}
