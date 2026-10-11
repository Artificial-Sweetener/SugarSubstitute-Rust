// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep completion evidence scoped to one quiescent operation, before native preparation.
use super::*;
use crate::runtime_test_support::{Rig, TestResult};
#[test]
fn previous_completion_cannot_survive_next_preparation_failure_or_live_reuse() -> TestResult {
    let rig = Rig::new()?;
    let mut operation = Operation::new(Signal::new(&rig.client.core));
    operation.completed = Some(4);
    operation.begin_reuse()?;
    // Any following CreateEvent/ResetEvent failure sees no previous successful count.
    assert_eq!(operation.take_completed(), None);
    operation.completed = Some(4);
    assert_eq!(operation.take_completed(), Some(4));
    assert_eq!(operation.take_completed(), None);
    operation.pending = true;
    operation.completed = Some(3);
    assert!(operation.begin_reuse().is_err());
    assert_eq!(operation.completed, Some(3));
    assert_eq!(operation.take_completed(), None);
    Ok(())
}
