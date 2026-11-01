// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canvas request lifecycle records cross the framed stderr relay under their own budget.
use super::*;
const LINE: &[u8] = b"2026-11-01T04:14:13.097161Z  INFO sugarsubstitute::canvas_surface_diagnostic: operation=\"canvas_surface_request\" schema=1 stage=Admitted op=Overview request_id=115 revision=19 generation=4 attachment=\"5321edfd\" pending=None wait=None intent=0 current=0 status=None\n";
const READINESS: &[u8] = b"INFO sugarsubstitute::production::startup_readiness_diagnostic: operation=\"startup_readiness_acknowledged\" schema=1 launch=1 navigation=2\n";

#[test]
fn canvas_records_survive_chunking_and_leave_the_shared_phase_cap_intact() {
    for split in 1..LINE.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &LINE[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &LINE[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Canvas(record)]
            if record.request_id == 115 && record.revision == 19 && record.generation == 4));
    }
    let mut owner = Diagnostic::new();
    let mut canvas = 0_usize;
    let mut readiness = 0_usize;
    let mut sequence = 0;
    for _ in 0..usize::from(CANVAS_RECORDS) + 8 {
        sequence += 1;
        owner.feed(OutputStream::Stderr, sequence, false, false, LINE, |v| {
            canvas += usize::from(matches!(v, Observation::Canvas(_)));
        });
    }
    for _ in 0..40 {
        sequence += 1;
        owner.feed(OutputStream::Stderr, sequence, false, false, READINESS, |v| {
            readiness += usize::from(matches!(v, Observation::Readiness(_)));
        });
    }
    assert_eq!(canvas, usize::from(CANVAS_RECORDS));
    assert_eq!(readiness, 30);
}
