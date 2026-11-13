// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate actual original encoded extent at the synchronous returned-buffer boundary.
use super::*;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;
use sugarsubstitute_contracts::execution::Generation;
use sugarsubstitute_contracts::{lifecycle::*, startup::*};
fn message() -> Result<WireMessage<'static>> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    Ok(WireMessage {
        header: WireHeader {
            run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(2)?),
            deadline_qpc: 100,
        },
        payload: WirePayload::Acquire {
            identity: MainIdentity {
                run,
                launch: LaunchId::new(1)?,
                navigation: Generation::new(1)?,
            },
            target:
                sugarsubstitute_supervision::lifecycle_protocol::BorrowedBackendTarget::Attached(
                    LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
                ),
        },
    })
}
#[test]
fn acquisition_native_returned_expiry_preserves_only_exact_encoded_scope_and_typed_cause()
-> Result<()> {
    let m = message()?;
    let mut bytes = [0; 512];
    let n = FrameCodec::encode(&m, &mut bytes)?;
    let original = Moment::from_nanos(100);
    let cause = NativeWriteExpiry::from_returned(
        &m,
        original,
        LifecyclePipeError::DeadlineExpired,
        &bytes[..n],
    )?;
    assert_eq!(cause.header, m.header);
    assert_eq!(cause.original, original);
    assert_eq!(cause.purpose, Purpose::Acquisition);
    assert_eq!(
        cause.source().and_then(|e| e.downcast_ref::<LifecyclePipeError>()),
        Some(&LifecyclePipeError::DeadlineExpired)
    );
    assert!(
        NativeWriteExpiry::from_returned(&m, original, LifecyclePipeError::Busy, &bytes[..n])
            .is_err()
    );
    assert!(
        NativeWriteExpiry::from_returned(&m, original, LifecyclePipeError::Closed, &bytes[..n])
            .is_err()
    );
    let mut foreign = message()?;
    foreign.header.deadline_qpc += 1;
    assert!(
        NativeWriteExpiry::from_returned(
            &foreign,
            original,
            LifecyclePipeError::DeadlineExpired,
            &bytes[..n]
        )
        .is_err()
    );
    assert!(
        NativeWriteExpiry::from_returned(
            &m,
            original,
            LifecyclePipeError::DeadlineExpired,
            &bytes[..n - 1]
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn acquisition_native_returned_expiry_rejects_ordinary_or_changed_payload() -> Result<()> {
    let m = message()?;
    let ordinary = WireMessage {
        header: m.header,
        payload: WirePayload::MainRetired(MainIdentity {
            run: m.header.run,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        }),
    };
    let mut bytes = [0; 512];
    let n = FrameCodec::encode(&ordinary, &mut bytes)?;
    assert!(
        NativeWriteExpiry::from_returned(
            &m,
            Moment::from_nanos(100),
            LifecyclePipeError::DeadlineExpired,
            &bytes[..n]
        )
        .is_err()
    );
    assert!(
        NativeWriteExpiry::from_returned(
            &ordinary,
            Moment::from_nanos(100),
            LifecyclePipeError::DeadlineExpired,
            &bytes[..n]
        )
        .is_err()
    );
    Ok(())
}
