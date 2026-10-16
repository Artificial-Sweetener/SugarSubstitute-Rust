// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep ordinary profile bytes strict while recovery preparation carries explicit grant correlation.
use super::*;
use crate::{
    lifecycle::{InstanceId, RunGeneration, RunKey},
    recovery_supervisor::RecoveryGrantId,
};
fn prepare(grant: Option<RecoveryGrantId>) -> Result<BackendCustodyFrame, Box<dyn Error>> {
    let key = key()?;
    Ok(BackendCustodyFrame {
        key,
        nonce: u64::MAX,
        command: BackendCustodyCommand::ProfilePrepare {
            profile: ProfileCustodyKey::new(
                RunKey {
                    owner: key.owner(),
                    instance: InstanceId::new(3)?,
                    generation: RunGeneration::new(7)?,
                },
                9,
            )?,
            recovery_grant: grant,
            job: 40,
            directory: 44,
            record: [8; 160],
            claim_identity: [6; 24],
            until_ticks: 100,
            frequency: 10,
        },
    })
}
#[test]
fn recovery_profile_prepare_has_distinct_canonical_tag_and_preserves_ordinary_bytes_and_complete_key()
-> TestResult {
    let ordinary = prepare(None)?;
    let granted = prepare(Some(RecoveryGrantId::new(u64::MAX)?))?;
    let ordinary_bytes = ordinary.encode()?;
    let granted_bytes = granted.encode()?;
    assert_eq!(ordinary_bytes[10], 12);
    assert_eq!(granted_bytes[10], 20);
    assert_eq!(ordinary_bytes[320..], [0; BACKEND_CUSTODY_FRAME_BYTES - 320]);
    assert_eq!(granted_bytes[320..328], u64::MAX.to_le_bytes());
    assert_eq!(granted_bytes[328..], [0; BACKEND_CUSTODY_FRAME_BYTES - 328]);
    let mut matched = granted_bytes;
    matched[10] = 12;
    matched[320..328].fill(0);
    assert_eq!(matched, ordinary_bytes);
    for frame in [ordinary, granted] {
        let decoded = BackendCustodyFrame::decode(&frame.encode()?)?;
        assert_eq!(decoded, frame);
        assert_eq!(decoded.key, key()?);
        assert_eq!(decoded.nonce, u64::MAX);
    }
    let mut downgraded = granted_bytes;
    downgraded[10] = 12;
    assert_eq!(BackendCustodyFrame::decode(&downgraded), Err(BackendCustodyWireError::Shape));
    let mut missing_grant = ordinary_bytes;
    missing_grant[10] = 20;
    assert_eq!(BackendCustodyFrame::decode(&missing_grant), Err(BackendCustodyWireError::Identity));
    let next_session = BackendCustodyFrame {
        key: BackendCustodyKey::new(ordinary.key.owner(), ordinary.key.session().next()?),
        nonce: 1,
        ..granted
    };
    assert_ne!(next_session.key, ordinary.key);
    assert_eq!(BackendCustodyFrame::decode(&next_session.encode()?)?, next_session);
    Ok(())
}
#[test]
fn recovery_profile_prepare_rejects_grant_downgrade_padding_invalid_owned_facts_and_foreign_owner()
-> TestResult {
    for grant in [None, Some(RecoveryGrantId::new(17)?)] {
        let frame = prepare(grant)?;
        let bytes = frame.encode()?;
        let reserved_start = if grant.is_some() { 328 } else { 320 };
        for offset in (11..16).chain(reserved_start..BACKEND_CUSTODY_FRAME_BYTES) {
            let mut changed = bytes;
            changed[offset] = 1;
            assert_eq!(
                BackendCustodyFrame::decode(&changed),
                Err(BackendCustodyWireError::Shape),
                "reserved offset {offset}"
            );
        }
        for (offset, width) in
            [(72usize, 8usize), (96, 8), (104, 8), (112, 8), (120, 8), (128, 8), (296, 24)]
        {
            let mut changed = bytes;
            changed[offset..offset + width].fill(0);
            assert_eq!(
                BackendCustodyFrame::decode(&changed),
                Err(BackendCustodyWireError::Identity),
                "owned fact offset {offset}"
            );
        }
        assert_eq!(
            BackendCustodyFrame::decode(&bytes[..BACKEND_CUSTODY_FRAME_BYTES - 1]),
            Err(BackendCustodyWireError::Shape)
        );
        let mut extended = bytes.to_vec();
        extended.push(0);
        assert_eq!(BackendCustodyFrame::decode(&extended), Err(BackendCustodyWireError::Shape));
        let mut invalid_handle = bytes;
        invalid_handle[104..112].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            BackendCustodyFrame::decode(&invalid_handle),
            Err(BackendCustodyWireError::Identity)
        );
        let BackendCustodyCommand::ProfilePrepare {
            profile,
            recovery_grant,
            job,
            directory,
            record,
            claim_identity,
            until_ticks,
            frequency,
        } = frame.command
        else {
            return Err("Actual profile prepare missing".into());
        };
        let foreign = ProfileCustodyKey::new(
            RunKey {
                owner: CoordinatorKey { namespace: NamespaceId::new([7; 32]), ..frame.key.owner() },
                ..profile.run()
            },
            profile.registry_process(),
        )?;
        let rejected = BackendCustodyFrame {
            command: BackendCustodyCommand::ProfilePrepare {
                profile: foreign,
                recovery_grant,
                job,
                directory,
                record,
                claim_identity,
                until_ticks,
                frequency,
            },
            ..frame
        };
        assert_eq!(rejected.encode(), Err(BackendCustodyWireError::Identity));
    }
    Ok(())
}
