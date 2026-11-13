// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the same retained ledger with actual decoded native reply observations.
use super::super::{
    native_abi::NativeCaptureReply,
    native_reply::{CauseDomain, decode},
};
use super::*;

/// One real-shaped failed reply with the exact original Win32 domain.
fn original() -> NativeCaptureReply {
    NativeCaptureReply {
        version: 2,
        status: 2,
        attach: 71,
        primary_error: 5,
        primary_domain: 1,
        ..NativeCaptureReply::default()
    }
}

/// Decode the actual owner's reply; fixture values grant no native custody.
fn observed(
    reply: &NativeCaptureReply,
) -> Result<ReplyFacts, super::super::native_reply::ReplyError> {
    decode(reply)
}

#[test]
fn repeated_primary_observation_does_not_invent_secondary_causes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = NativeFailureLedger::default();
    let facts = observed(&original())?;
    ledger.adopt(&facts);
    ledger.adopt(&facts);
    assert_eq!(ledger.count, 0);
    assert_eq!(ledger.omitted, 0);
    assert_eq!(ledger.primary, facts.primary);
    Ok(())
}

#[test]
fn consumed_nested_and_outer_secondary_observations_both_survive()
-> Result<(), Box<dyn std::error::Error>> {
    let mut nested = original();
    nested.cleanup_count = 1;
    nested.cleanup_codes[0] = 13;
    nested.cleanup_domains = 3;
    let mut outer = original();
    outer.cleanup_count = 1;
    outer.cleanup_codes[0] = 0xc0000022;
    outer.cleanup_domains = 2;
    let mut ledger = NativeFailureLedger::default();
    ledger.adopt(&observed(&nested)?);
    ledger.adopt(&observed(&outer)?);
    assert_eq!(ledger.count, 2);
    assert_eq!(ledger.secondary[0], Some(NativeCause { domain: CauseDomain::Errno, code: 13 }));
    assert_eq!(
        ledger.secondary[1],
        Some(NativeCause { domain: CauseDomain::NtStatus, code: 0xc0000022 })
    );
    assert_eq!(
        ledger.failure().and_then(|failure| failure.source().map(ToString::to_string)),
        Some("Capture Win32 cause 0x00000005".to_owned())
    );
    Ok(())
}

#[test]
fn ninth_actual_secondary_is_counted_without_overwriting_the_first_eight()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = original();
    reply.cleanup_count = 8;
    reply.cleanup_domains = 0x5555;
    reply.cleanup_codes = [6, 7, 8, 9, 10, 11, 12, 13];
    let mut ledger = NativeFailureLedger::default();
    ledger.adopt(&observed(&reply)?);
    let original_slots = ledger.secondary;
    reply.cleanup_count = 1;
    reply.cleanup_domains = 1;
    reply.cleanup_codes = [14, 0, 0, 0, 0, 0, 0, 0];
    ledger.adopt(&observed(&reply)?);
    assert_eq!(ledger.secondary, original_slots);
    assert_eq!(ledger.omitted, 1);
    Ok(())
}

#[test]
fn original_omission_count_and_new_overflow_saturate_without_wrap()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = original();
    reply.omitted_cleanup = u64::MAX;
    let mut ledger = NativeFailureLedger::default();
    ledger.adopt(&observed(&reply)?);
    reply.omitted_cleanup = 1;
    ledger.adopt(&observed(&reply)?);
    assert_eq!(ledger.omitted, u64::MAX);
    Ok(())
}

#[test]
fn a_later_different_primary_never_replaces_the_original_producer_cause()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = NativeFailureLedger::default();
    ledger.adopt(&observed(&original())?);
    let mut later = original();
    later.primary_error = 13;
    later.primary_domain = 3;
    ledger.adopt(&observed(&later)?);
    assert_eq!(ledger.primary, Some(NativeCause { domain: CauseDomain::Win32, code: 5 }));
    assert_eq!(ledger.secondary[0], Some(NativeCause { domain: CauseDomain::Errno, code: 13 }));
    Ok(())
}

#[test]
fn no_native_cause_is_manufactured_before_an_actual_failed_reply()
-> Result<(), Box<dyn std::error::Error>> {
    let ledger = NativeFailureLedger::default();
    assert!(ledger.failure().is_none());
    Ok(())
}
