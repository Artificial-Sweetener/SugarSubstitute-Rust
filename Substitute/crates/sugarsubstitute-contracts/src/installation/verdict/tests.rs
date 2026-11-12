// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Each trial ending maps to one verdict, and the first ending fact decides.
use super::*;

type Step = fn(&mut TrialFacts);

fn facts(steps: &[Step]) -> TrialFacts {
    let mut facts = TrialFacts::default();
    for step in steps {
        step(&mut facts);
    }
    facts
}
fn host(facts: &mut TrialFacts) {
    facts.observe_failure(FailedProcess::Host);
}
fn backend(facts: &mut TrialFacts) {
    facts.observe_failure(FailedProcess::Backend);
}
fn close(facts: &mut TrialFacts) {
    facts.observe_close(CloseCause::Requested);
}
fn session(facts: &mut TrialFacts) {
    facts.observe_close(CloseCause::SessionEnd);
}
fn ready(facts: &mut TrialFacts) {
    facts.observe_readiness();
}

#[test]
fn only_the_hosts_own_startup_failure_is_bad() {
    use TrialVerdict::{Bad, NoVerdict};
    let cases: [(&[Step], TrialVerdict); 10] = [
        (&[host], Bad(BadTrial::StartupFailed)),
        (&[host, close], Bad(BadTrial::StartupFailed)),
        (&[backend], NoVerdict(Undecided::BackendFailed)),
        (&[backend, host], NoVerdict(Undecided::BackendFailed)),
        (&[close], NoVerdict(Undecided::CloseRequested)),
        (&[close, host], NoVerdict(Undecided::CloseRequested)),
        (&[session, host], NoVerdict(Undecided::SessionEnd)),
        (&[], NoVerdict(Undecided::CloseRequested)),
        (&[ready, host], NoVerdict(Undecided::AcceptanceUnpublished)),
        (&[host, ready], NoVerdict(Undecided::AcceptanceUnpublished)),
    ];
    for (steps, verdict) in cases {
        assert_eq!(facts(steps).verdict(), verdict);
    }
}

#[test]
fn the_witness_calls_only_crashes_panics_and_hangs_bad() {
    use TrialVerdict::{Bad, NoVerdict};
    for (exit, verdict) in [
        (HostExit::Finished, NoVerdict(Undecided::CloseRequested)),
        (HostExit::Ended(0xC000_0005), Bad(BadTrial::Crashed)),
        (HostExit::Ended(0xC000_0409), Bad(BadTrial::Crashed)),
        (HostExit::Ended(101), Bad(BadTrial::Crashed)),
        (HostExit::Ended(UNRESPONSIVE_EXIT_STATUS), Bad(BadTrial::Unresponsive)),
        (HostExit::Ended(1), NoVerdict(Undecided::ProcessEnded)),
        (HostExit::Ended(0), NoVerdict(Undecided::ProcessEnded)),
        (HostExit::Ended(0x4001_0004), NoVerdict(Undecided::ProcessEnded)),
        (HostExit::Ended(0x8000_0003), NoVerdict(Undecided::ProcessEnded)),
    ] {
        assert_eq!(exit.verdict(), verdict, "{exit:?}");
    }
}
