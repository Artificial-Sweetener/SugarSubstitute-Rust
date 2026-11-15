// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded native ABI bytes without invoking Windows security APIs.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
const ACCOUNT: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 42, 0, 0, 0];
#[test]
fn expected_descriptor_has_exact_owner_protected_full_access_and_medium_label() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    assert!(bytes.len() < 512);
    assert_eq!(owner(&bytes)?, ACCOUNT);
    assert_eq!(short(&bytes, 2)?, CONTROL);
    let dacl = offset(&bytes, 16)?;
    let sacl = offset(&bytes, 12)?;
    assert_eq!(short(&bytes, dacl + 4)?, 2);
    assert_eq!(short(&bytes, sacl + 4)?, 1);
    assert_eq!(&bytes[dacl + 8..dacl + 12], &[0, 3, 24, 0]);
    assert_eq!(&bytes[sacl + 8..sacl + 12], &[17, 0, 20, 0]);
    verify(&bytes, ACCOUNT)?;
    Ok(())
}
#[test]
fn changed_owner_acl_label_or_unprotected_descriptor_fails_closed() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    let dacl = offset(&bytes, 16)?;
    let sacl = offset(&bytes, 12)?;
    let owner_offset = offset(&bytes, 4)?;
    // Owner, the account's rights, the account's SID, label type, label policy, label authority.
    for index in [owner_offset + 8, dacl + 12, dacl + 16, sacl + 8, sacl + 12, sacl + 20] {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    let mut changed = bytes.clone();
    changed[3] &= !0x10;
    assert!(verify(&changed, ACCOUNT).is_err());
    Ok(())
}
#[test]
fn malformed_foreign_ranges_ace_counts_and_capacity_reject_before_copying() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    let dacl = offset(&bytes, 16)?;
    for field in [4, 12, 16] {
        let mut changed = bytes.clone();
        changed[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    for (field, value) in [(dacl + 2, u16::MAX), (dacl + 4, u16::MAX), (dacl + 10, u16::MAX)] {
        let mut changed = bytes.clone();
        changed[field..field + 2].copy_from_slice(&value.to_le_bytes());
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    assert!(verify(&vec![0; MAXIMUM_BYTES + 1], ACCOUNT).is_err());
    for length in 0..bytes.len() {
        assert!(verify(&bytes[..length], ACCOUNT).is_err());
    }
    assert!(expected(&[1; 69]).is_err());
    Ok(())
}

#[test]
fn recovery_job_descriptor_cannot_be_substituted_for_file_security()
-> Result<(), Box<dyn std::error::Error>> {
    let sid = &[1, 1, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0];
    let file = expected(sid)?;
    let job = expected_recovery_job(sid)?;
    verify(&file, sid)?;
    verify_recovery_job(&job, sid)?;
    assert!(verify(&job, sid).is_err());
    assert!(verify_recovery_job(&file, sid).is_err());
    let foreign = &[1, 1, 0, 0, 0, 0, 0, 5, 22, 0, 0, 0];
    assert!(verify_recovery_job(&job, foreign).is_err());
    let mut unprotected = job;
    unprotected[3] &= !0x10;
    assert!(verify_recovery_job(&unprotected, sid).is_err());
    Ok(())
}

/// A self-relative descriptor with `aces` as its DACL and `label` as its SACL, if any.
fn descriptor(control: u16, dacl: &[Vec<u8>], label: Option<Vec<u8>>) -> Vec<u8> {
    let dacl = acl(dacl);
    let mut bytes = vec![0; 20];
    bytes[0] = 1;
    bytes[2..4].copy_from_slice(&control.to_le_bytes());
    let mut parts = vec![(4, ACCOUNT.to_vec()), (16, dacl)];
    if let Some(label) = label {
        parts.push((12, acl(&[label])));
    }
    for (field, part) in parts {
        let position = bytes.len() as u32;
        bytes[field..field + 4].copy_from_slice(&position.to_le_bytes());
        bytes.extend_from_slice(&part);
    }
    bytes
}

const AUTO_INHERITED: u16 = 0x0400 | 0x0800;
const USERS: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 33, 2, 0, 0];
const LOW: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 16, 0, 16, 0, 0];

#[test]
fn a_permission_tool_round_trip_that_keeps_the_same_access_is_still_trusted() -> TestResult {
    // What icacls and a restore left on the root entry: SYSTEM first, no inheritance flags,
    // the auto-inherited bits set and the label kept.
    let label = Some(ace(17, 0, 1, MEDIUM));
    let reordered = descriptor(
        CONTROL | AUTO_INHERITED,
        &[ace(0, 0, 0x001f01ff, SYSTEM), ace(0, 0, 0x001f01ff, ACCOUNT)],
        label.clone(),
    );
    verify(&reordered, ACCOUNT)?;
    // A denial for the account, such as "deny execute", only takes access away.
    let denied = descriptor(
        CONTROL | AUTO_INHERITED,
        &[ace(1, 0, 0x20, ACCOUNT), ace(0, 3, 0x001f01ff, ACCOUNT), ace(0, 3, 0x001f01ff, SYSTEM)],
        label,
    );
    verify(&denied, ACCOUNT)?;
    // A backup that restores only the DACL leaves no label: the object is medium by default.
    let unlabelled = descriptor(CONTROL & !0x0010, &[ace(0, 3, 0x001f01ff, ACCOUNT)], None);
    verify(&unlabelled, ACCOUNT)?;
    Ok(())
}

#[test]
fn security_that_lets_another_account_or_a_lower_integrity_in_is_refused() -> TestResult {
    let label = Some(ace(17, 0, 1, MEDIUM));
    let ours = [ace(0, 3, 0x001f01ff, ACCOUNT), ace(0, 3, 0x001f01ff, SYSTEM)];
    let foreign = [ours[0].clone(), ace(0, 0, 0x0012019f, USERS)];
    assert!(verify(&descriptor(CONTROL, &foreign, label.clone()), ACCOUNT).is_err());
    let inherited = [ours[0].clone(), ace(0, 0x10, 0x001301bf, USERS)];
    assert!(verify(&descriptor(CONTROL, &inherited, label.clone()), ACCOUNT).is_err());
    assert!(verify(&descriptor(CONTROL & !0x1000, &ours, label.clone()), ACCOUNT).is_err());
    assert!(verify(&descriptor(CONTROL, &ours, Some(ace(17, 0, 1, LOW))), ACCOUNT).is_err());
    assert!(verify(&descriptor(CONTROL, &ours, Some(ace(17, 0, 0, MEDIUM))), ACCOUNT).is_err());
    let partial = [ace(0, 3, 0x00120089, ACCOUNT), ours[1].clone()];
    assert!(verify(&descriptor(CONTROL, &partial, label.clone()), ACCOUNT).is_err());
    let mut null_dacl = descriptor(CONTROL, &ours, label);
    null_dacl[16..20].copy_from_slice(&[0; 4]);
    assert!(verify(&null_dacl, ACCOUNT).is_err());
    Ok(())
}

#[test]
fn a_generic_all_grant_is_full_access_and_harmless_inheritance_is_repairable() -> TestResult {
    let label = Some(ace(17, 0, 1, MEDIUM));
    let generic = descriptor(
        CONTROL,
        &[ace(0, 0, 0x1000_0000, ACCOUNT), ace(0, 0, 0x001f01ff, SYSTEM)],
        label.clone(),
    );
    verify(&generic, ACCOUNT)?;
    // Inherited grants to the account and SYSTEM only: protecting the DACL again repairs it.
    let ours = [ace(0, 0x10, 0x001f01ff, ACCOUNT), ace(0, 0x10, 0x001f01ff, SYSTEM)];
    let inheriting = descriptor(CONTROL & !0x1000, &ours, label.clone());
    assert_eq!(verify(&inheriting, ACCOUNT), Err(UNPROTECTED));
    let foreign = [ours[0].clone(), ace(0, 0x10, 0x0012019f, USERS)];
    let open = descriptor(CONTROL & !0x1000, &foreign, label);
    assert_eq!(verify(&open, ACCOUNT), Err("foreign_storage_security"));
    Ok(())
}
