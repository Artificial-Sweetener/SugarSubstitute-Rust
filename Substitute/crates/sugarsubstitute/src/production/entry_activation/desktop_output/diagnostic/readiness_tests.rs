// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The existing framed stderr relay preserves bounded acknowledgement facts across chunking.
use super::*;
const LINE: &[u8] = b"INFO sugarsubstitute::production::startup_readiness_diagnostic: operation=\"startup_readiness_acknowledged\" schema=1 launch=1 navigation=2\n";

#[test]
fn readiness_relay_requires_complete_contiguous_stderr_and_keeps_the_shared_cap() {
    for split in 1..LINE.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &LINE[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &LINE[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Readiness(record)]
            if record.launch.get() == 1 && record.navigation.get() == 2));
    }
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stdout, 1, false, false, LINE, |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 1, false, false, &LINE[..20], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 3, true, false, &LINE[20..], |v| seen.push(v));
    assert!(seen.is_empty());
    for sequence in 4..=40 {
        owner.feed(OutputStream::Stderr, sequence, false, false, LINE, |v| seen.push(v));
    }
    assert_eq!(seen.len(), 30);
    assert!(seen.iter().all(|v| matches!(v, Observation::Readiness(_))));
}
