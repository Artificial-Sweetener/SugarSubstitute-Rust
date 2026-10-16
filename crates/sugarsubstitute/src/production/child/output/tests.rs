// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original output custody and native polling deadlines.
use super::*;
#[test]
fn close_keeps_pending_pull_with_output_owner_and_blocks_early_frame_release() -> Result<()> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let header = WireHeader {
        run,
        connection: Some(ConnectionId::new(1)?),
        request: Some(RequestId::new(2)?),
        deadline_qpc: 100,
    };
    let mut output = ChildOutput::new();
    output.pending = Some((header, Moment::from_nanos(100)));
    output.pull_after = Some(Moment::from_nanos(50));
    output.close();
    assert_eq!(output.pull_after, None);
    assert!(output.closing() && output.pending());
    assert!(output.owns_reply(header));
    assert!(!output.owns_reply(WireHeader { request: Some(RequestId::new(3)?), ..header }));
    assert!(output.release_frame().is_err());
    assert_eq!(output.deadline(), Some(Moment::from_nanos(100)));
    assert_eq!(output.pending, Some((header, Moment::from_nanos(100))));
    Ok(())
}

#[path = "polling_tests.rs"]
mod polling_tests;
