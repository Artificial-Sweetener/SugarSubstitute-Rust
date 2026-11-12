// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode actual fixed native inputs without granting native output or cleanup authority.
use super::*;

/// An initialized actual attaching reply contains no fabricated registration or closure.
fn attaching() -> NativeCaptureReply {
    NativeCaptureReply {
        version: 2,
        status: 5,
        attach: 71,
        frequency: 1000,
        ..NativeCaptureReply::default()
    }
}
/// Extract the typed reject cause without requiring facts to implement Debug.
fn rejected(reply: &NativeCaptureReply) -> Result<ReplyError, &'static str> {
    match decode(reply) {
        Ok(_) => Err("Malformed native reply was accepted"),
        Err(cause) => Ok(cause),
    }
}
#[test]
fn initialized_attaching_reply_has_only_inert_correlated_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let facts = decode(&attaching())?;
    assert_eq!(facts.status, 5);
    assert!(!facts.registered && !facts.pending && !facts.retired);
    assert!(facts.primary.is_none() && facts.cleanup.is_none());
    Ok(())
}
#[test]
fn exact_v2_layout_and_offsets_match_the_native_export() -> Result<(), Box<dyn std::error::Error>> {
    use super::super::native_abi::*;
    assert_eq!(size_of::<NativeCaptureInput>(), 776);
    assert_eq!(size_of::<NativeCaptureProgress>(), 48);
    assert_eq!(size_of::<NativeCaptureStep>(), 32);
    assert_eq!(size_of::<NativeCaptureReply>(), 120);
    assert_eq!(std::mem::offset_of!(NativeCaptureReply, attach), 24);
    assert_eq!(std::mem::offset_of!(NativeCaptureReply, primary_domain), 68);
    assert_eq!(std::mem::offset_of!(NativeCaptureReply, omitted_cleanup), 72);
    Ok(())
}
#[test]
fn v1_unknown_versions_and_unknown_statuses_are_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    for version in [0, 1, 3, u32::MAX] {
        let mut reply = attaching();
        reply.version = version;
        assert_eq!(rejected(&reply)?, ReplyError::Version);
    }
    for status in [6, u32::MAX] {
        let mut reply = attaching();
        reply.status = status;
        assert_eq!(rejected(&reply)?, ReplyError::Status);
    }
    Ok(())
}
#[test]
fn native_boolean_fields_require_exact_zero_or_one() -> Result<(), Box<dyn std::error::Error>> {
    for field in 0..3 {
        let mut reply = attaching();
        match field {
            0 => reply.registered = 2,
            1 => reply.pending_io = 2,
            _ => reply.retired = 2,
        }
        assert_eq!(rejected(&reply)?, ReplyError::Boolean);
    }
    Ok(())
}
#[test]
fn retirement_and_pending_io_cannot_both_be_true() -> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.status = 4;
    reply.retired = 1;
    reply.pending_io = 1;
    assert_eq!(rejected(&reply)?, ReplyError::ContradictoryFacts);
    Ok(())
}
#[test]
fn registered_or_retired_reply_requires_original_owner_token()
-> Result<(), Box<dyn std::error::Error>> {
    for registered in [false, true] {
        let mut reply = attaching();
        reply.attach = 0;
        reply.status = if registered { 1 } else { 4 };
        reply.registered = u32::from(registered);
        reply.retired = u32::from(!registered);
        assert_eq!(rejected(&reply)?, ReplyError::ContradictoryFacts);
    }
    Ok(())
}
#[test]
fn initial_status_cannot_claim_an_existing_native_owner() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reply = attaching();
    reply.status = 0;
    assert_eq!(rejected(&reply)?, ReplyError::ContradictoryFacts);
    Ok(())
}
#[test]
fn registered_status_requires_registered_and_unretired_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.status = 1;
    assert_eq!(rejected(&reply)?, ReplyError::ContradictoryFacts);
    reply.registered = 1;
    assert!(decode(&reply).is_ok());
    reply.retired = 1;
    assert_eq!(rejected(&reply)?, ReplyError::ContradictoryFacts);
    Ok(())
}
#[test]
fn win32_ntstatus_and_errno_remain_distinct_original_domains()
-> Result<(), Box<dyn std::error::Error>> {
    for (domain, code, expected) in [
        (1, 5, CauseDomain::Win32),
        (2, 0xc0000022, CauseDomain::NtStatus),
        (3, 13, CauseDomain::Errno),
    ] {
        let mut reply = attaching();
        reply.status = 2;
        reply.primary_error = code;
        reply.primary_domain = domain;
        let facts = decode(&reply)?;
        assert_eq!(facts.primary, Some(NativeCause { domain: expected, code }));
    }
    Ok(())
}
#[test]
fn zero_mismatched_domain_success_ntstatus_and_invalid_errno_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    for (domain, code) in [(1, 0), (0, 5), (4, 5), (2, 1), (3, 0x80000000)] {
        let mut reply = attaching();
        reply.status = 2;
        reply.primary_error = code;
        reply.primary_domain = domain;
        assert_eq!(rejected(&reply)?, ReplyError::CauseDomain);
    }
    Ok(())
}
#[test]
fn all_eight_mixed_secondary_causes_and_omitted_count_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.status = 2;
    reply.primary_error = 5;
    reply.primary_domain = 1;
    reply.cleanup_count = 8;
    reply.omitted_cleanup = u64::MAX;
    let mut expected = [None; 8];
    for (index, output) in expected.iter_mut().enumerate() {
        let (domain, code, typed) = match index % 3 {
            0 => (1, 6, CauseDomain::Win32),
            1 => (2, 0xc0000008, CauseDomain::NtStatus),
            _ => (3, 22, CauseDomain::Errno),
        };
        reply.cleanup_codes[index] = code;
        reply.cleanup_domains |= domain << (index * 2);
        *output = Some(NativeCause { domain: typed, code });
    }
    let facts = decode(&reply)?;
    assert_eq!(facts.primary, Some(NativeCause { domain: CauseDomain::Win32, code: 5 }));
    assert_eq!(facts.secondary, expected);
    assert_eq!(facts.omitted, u64::MAX);
    Ok(())
}
#[test]
fn secondary_ledger_rejects_count_overflow_unused_bits_and_stale_slots()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.cleanup_count = 9;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupLedger);
    reply.cleanup_count = 0;
    reply.cleanup_domains = 1;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupLedger);
    reply.cleanup_domains = 0;
    reply.cleanup_codes[7] = 5;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupLedger);
    Ok(())
}
#[test]
fn secondary_slots_require_nonzero_codes_with_known_original_domains()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.cleanup_count = 1;
    reply.cleanup_domains = 1;
    assert_eq!(rejected(&reply)?, ReplyError::CauseDomain);
    reply.cleanup_codes[0] = 5;
    reply.cleanup_domains = 0;
    assert_eq!(rejected(&reply)?, ReplyError::CauseDomain);
    reply.cleanup_domains = 2;
    assert_eq!(rejected(&reply)?, ReplyError::CauseDomain);
    Ok(())
}
#[test]
fn original_cleanup_tuple_is_preserved_without_starting_a_new_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.cleanup_state = 1;
    reply.cleanup_started = 100;
    reply.cleanup_grace = 10_100;
    reply.cleanup_total = 15_100;
    let facts = decode(&reply)?;
    let cleanup = facts.cleanup.ok_or("Original tuple missing")?;
    assert_eq!(
        (cleanup.started, cleanup.grace, cleanup.total, cleanup.frequency),
        (100, 10_100, 15_100, 1000)
    );
    reply.cleanup_grace = 100;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupBudget);
    reply.cleanup_grace = 10_100;
    reply.frequency = 0;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupBudget);
    Ok(())
}
#[test]
fn dormant_cleanup_rejects_stale_tuple_and_unknown_state() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reply = attaching();
    reply.cleanup_started = 1;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupBudget);
    reply.cleanup_started = 0;
    reply.cleanup_state = 2;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupBudget);
    Ok(())
}
#[test]
fn checked_retired_fact_preserves_original_failure_without_claiming_global_release()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.status = 4;
    reply.retired = 1;
    reply.primary_error = 5;
    reply.primary_domain = 1;
    let facts = decode(&reply)?;
    assert!(facts.retired && !facts.pending);
    assert_eq!(facts.primary, Some(NativeCause { domain: CauseDomain::Win32, code: 5 }));
    Ok(())
}

#[test]
fn returned_secondary_or_omitted_causes_require_actual_primary()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = attaching();
    reply.cleanup_count = 1;
    reply.cleanup_codes[0] = 6;
    reply.cleanup_domains = 1;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupLedger);
    reply.cleanup_count = 0;
    reply.cleanup_codes[0] = 0;
    reply.cleanup_domains = 0;
    reply.omitted_cleanup = 1;
    assert_eq!(rejected(&reply)?, ReplyError::CleanupLedger);
    Ok(())
}
