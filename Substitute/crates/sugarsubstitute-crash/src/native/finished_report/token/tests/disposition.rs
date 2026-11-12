// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject incomplete native disposition before adopting its claimed token destruction.
use super::super::{abi, disposition::valid_disposition_output};

/// Supply the complete checked native success shape before independently corrupting each field.
fn complete() -> abi::DispositionOutput {
    let mut state = abi::Output::empty();
    state.version = 1;
    state.retired = 1;
    state.facts_valid = 1;
    state.reclaimed_bytes = 64;
    abi::DispositionOutput { state, source: 2, metadata: 2, stages_absent: 3, reserved: 0 }
}

#[test]
fn disposition_requires_checked_complete_retirement_and_zero_current_read_bytes() {
    assert!(valid_disposition_output(&complete(), false, 64));
    assert!(valid_disposition_output(&complete(), true, 64));
    for invalid in 0..12 {
        let mut output = complete();
        match invalid {
            0 => output.state.retired = 0,
            1 => output.state.lease = 91,
            2 => output.state.handles = 1,
            3 => output.state.locks = 1,
            4 => output.state.read_bytes = 64,
            5 => output.metadata = 3,
            6 => output.stages_absent = 1,
            7 => output.source = 4,
            8 => output.state.reclaimed_bytes = 63,
            9 => output.state.status = 7,
            10 => output.state.primary_error = 31,
            _ => output.state.cleanup_count = 1,
        }
        assert!(!valid_disposition_output(&output, false, 64), "case {invalid}");
    }
    let mut reconciled = complete();
    reconciled.source = 3;
    assert!(!valid_disposition_output(&reconciled, true, 64));
    reconciled.state.reclaimed_bytes = 0;
    assert!(valid_disposition_output(&reconciled, true, 64));
    let mut failed = complete();
    failed.state.status = 7;
    failed.state.primary_error = 8;
    failed.source = 4;
    failed.metadata = 3;
    failed.stages_absent = 0;
    failed.state.reclaimed_bytes = 0;
    failed.state.facts_valid = 0;
    assert!(!valid_disposition_output(&failed, false, 64));
    assert!(valid_disposition_output(&failed, true, 64));
    failed.state.retired = 0;
    failed.state.lease = 91;
    assert!(valid_disposition_output(&failed, false, 64));
}
