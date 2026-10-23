// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native splash visibility belongs to the Desktop, never to a lifecycle peer.
use super::*;

#[test]
fn lifecycle_peer_cannot_request_a_splash_visibility_handoff() -> TestResult {
    let mut source = encoded(WirePayload::MainRetired(main()?))?;
    source[12..14].copy_from_slice(&22u16.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::UnsupportedTag)));
    source[12..14].copy_from_slice(&21u16.to_le_bytes());
    source[4..6].copy_from_slice(&8u16.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Schema)));
    Ok(())
}
