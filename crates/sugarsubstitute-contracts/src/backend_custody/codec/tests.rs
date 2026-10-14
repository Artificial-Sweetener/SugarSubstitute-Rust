// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded correlation facts and reject every changed wire authority field.
use super::*;
type TestResult = Result<(), Box<dyn Error>>;
fn key() -> Result<BackendCustodyKey, Box<dyn Error>> {
    Ok(BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        BackendCustodySession::new(1)?,
    ))
}
#[test]
fn all_custody_commands_preserve_complete_matching_facts_in_exact_fixed_frames() -> TestResult {
    let key = key()?;
    let resource = ResourceKey {
        owner: key.owner(),
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let commands = [
        BackendCustodyCommand::Hello { capability: [3; 32] },
        BackendCustodyCommand::Establish { parent: 40, pid: 9, birth: 20, containment: 44 },
        BackendCustodyCommand::Prepare {
            resource,
            digest: [4; 32],
            job: 48,
            until_ticks: 100,
            frequency: 10,
            record: [5; 128],
            claim_identity: [6; 24],
            registry_process: 3,
        },
        BackendCustodyCommand::Root {
            resource,
            process: 52,
            pid: 11,
            birth: 21,
            registry_process: 3,
        },
        BackendCustodyCommand::Stop { resource, until_ticks: 200, frequency: 10 },
        BackendCustodyCommand::Finish { until_ticks: 200, frequency: 10 },
        BackendCustodyCommand::Receipt(BackendCustodyReceipt::Established),
        BackendCustodyCommand::Receipt(BackendCustodyReceipt::Prepared(resource)),
        BackendCustodyCommand::Receipt(BackendCustodyReceipt::Root(resource)),
        BackendCustodyCommand::Receipt(BackendCustodyReceipt::Released(resource)),
        BackendCustodyCommand::Receipt(BackendCustodyReceipt::Finished),
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let frame = BackendCustodyFrame { key, nonce: index as u64 + 1, command };
        assert_eq!(BackendCustodyFrame::decode(&frame.encode()?)?, frame);
    }
    Ok(())
}
#[test]
fn changed_padding_version_nonce_and_foreign_resource_never_become_native_commands() -> TestResult {
    let frame = BackendCustodyFrame {
        key: key()?,
        nonce: 1,
        command: BackendCustodyCommand::Finish { until_ticks: 10, frequency: 1 },
    };
    let bytes = frame.encode()?;
    for offset in [0, 8, 9, 11, 12, 15, 200, 383] {
        let mut changed = bytes;
        changed[offset] ^= 1;
        assert!(BackendCustodyFrame::decode(&changed).is_err());
    }
    let mut zero = bytes;
    zero[72..80].fill(0);
    assert!(BackendCustodyFrame::decode(&zero).is_err());
    assert!(BackendCustodyFrame::decode(&bytes[..383]).is_err());
    let foreign = ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: frame.key.owner().epoch,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    assert!(
        BackendCustodyFrame {
            command: BackendCustodyCommand::Receipt(BackendCustodyReceipt::Root(foreign)),
            ..frame
        }
        .encode()
        .is_err()
    );
    Ok(())
}
#[test]
fn invalid_destination_handles_and_counter_facts_fail_before_adoption() -> TestResult {
    let key = key()?;
    for raw in [0, u64::MAX, u64::MAX - 1] {
        assert!(
            BackendCustodyFrame {
                key,
                nonce: 1,
                command: BackendCustodyCommand::Establish {
                    parent: raw,
                    pid: 1,
                    birth: 1,
                    containment: 40
                }
            }
            .encode()
            .is_err()
        );
    }
    for (until_ticks, frequency) in [(0, 1), (-1, 1), (1, 0), (1, -1)] {
        assert!(
            BackendCustodyFrame {
                key,
                nonce: 1,
                command: BackendCustodyCommand::Finish { until_ticks, frequency }
            }
            .encode()
            .is_err()
        );
    }
    Ok(())
}
