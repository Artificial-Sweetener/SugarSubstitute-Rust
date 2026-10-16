// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Wrong callbacks and partial writes cannot manufacture a completed recovery event.
use super::*;
#[test]
fn complete_write_requires_actual_matching_issue_and_exact_bytes() -> Result<()> {
    let operation = LifecyclePipeOperationId::new(3)?;
    let other = LifecyclePipeOperationId::new(4)?;
    let mut owner = Receipt::new(operation, 2, 224);
    assert!(owner.written(operation, 224, 1).is_err());
    assert!(owner.issued(other).is_err());
    owner.issued(operation)?;
    assert!(owner.issued(operation).is_err());
    assert!(owner.written(other, 224, 1).is_err());
    assert!(owner.written(operation, 223, 1).is_err());
    assert!(owner.written(operation, 225, 1).is_err());
    owner.written(operation, 224, 1)?;
    assert!(owner.returned(operation).is_err());
    Ok(())
}
#[test]
fn replay_gap_and_returned_unissued_buffers_never_complete() -> Result<()> {
    let operation = LifecyclePipeOperationId::new(7)?;
    let mut owner = Receipt::new(operation, 8, 192);
    owner.returned(operation)?;
    assert!(owner.returned(operation).is_err());
    assert!(owner.issued(operation).is_err());
    assert!(owner.written(operation, 192, 7).is_err());
    let mut owner = Receipt::new(operation, 8, 192);
    owner.issued(operation)?;
    for previous in [0, 6, 8, u64::MAX] {
        assert!(owner.written(operation, 192, previous).is_err());
    }
    owner.written(operation, 192, 7)?;
    Ok(())
}
