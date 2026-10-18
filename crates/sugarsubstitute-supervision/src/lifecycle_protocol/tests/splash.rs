// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Version the exact covered-surface request without creating presentation authority.
use super::*;

#[test]
fn covered_splash_request_has_explicit_kind_identity_and_coordinated_schema() -> TestResult {
    let identity = main()?;
    let mut source = encoded(WirePayload::SplashPrepared(identity))?;
    assert_eq!(source.len(), HEADER_BYTES + 16);
    assert_eq!(&source[4..6], &8u16.to_le_bytes());
    assert_eq!(&source[12..14], &21u16.to_le_bytes());
    assert!(
        matches!(FrameCodec::decode(&source)?.payload, DecodedPayload::SplashPrepared(actual) if actual == identity)
    );
    source[4..6].copy_from_slice(&7u16.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Schema)));
    Ok(())
}

#[test]
fn covered_splash_cannot_cross_run_or_omit_request_or_positive_attempt() -> TestResult {
    let mut identity = main()?;
    identity.run.instance = InstanceId::new(99)?;
    let mut output = [0; 256];
    assert!(matches!(
        FrameCodec::encode(
            &WireMessage { header: header(21)?, payload: WirePayload::SplashPrepared(identity) },
            &mut output
        ),
        Err(ProtocolError::Identity)
    ));
    let original = encoded(WirePayload::SplashPrepared(main()?))?;
    for range in [88..96, 104..112, 112..120] {
        let mut changed = original.clone();
        changed[range].fill(0);
        assert!(matches!(FrameCodec::decode(&changed), Err(ProtocolError::Identity)));
    }
    Ok(())
}
