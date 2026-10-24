// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hostile message validation and exact native-entry remapping without native operations.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A lexical request supplies no native authority and must never be executed by these tests.
fn fixture() -> Request {
    Request {
        version: 1,
        id: [7; 32],
        executable: r"E:\application\sugarsubstitute.exe".into(),
        directory: r"E:\application".into(),
        entry: Entry::Installed { selection: 12 },
        pins: vec![16, 20],
    }
}

#[test]
fn request_round_trip_rebinds_arguments_to_receiver_handles() -> TestResult {
    let request = fixture();
    let decoded = Request::decode(&request.encode()?, [7; 32])?;
    assert_eq!(
        decoded.entry.arguments(&[44])?,
        [OsString::from(INSTALLED_LAUNCH_ARGUMENT), "44".into()]
    );
    assert_eq!(
        Entry::Trial { run: 4, snapshot: 8 }.arguments(&[32, 36])?,
        [OsString::from(TRIAL_LAUNCH_ARGUMENT), "32".into(), "36".into()]
    );
    assert!(decoded.entry.arguments(&[44, 48]).is_err());
    assert!(Entry::Trial { run: 4, snapshot: 8 }.arguments(&[32]).is_err());
    Ok(())
}

#[test]
fn malformed_unsupported_and_foreign_requests_never_grant_a_command() -> TestResult {
    let original = serde_json::to_value(fixture())?;
    for (key, value) in [
        ("version", serde_json::json!(2)),
        ("id", serde_json::json!(vec![0; 32])),
        ("executable", serde_json::json!("relative.exe")),
        ("directory", serde_json::json!("E:relative")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut wire = original.clone();
        wire[key] = value;
        assert!(Request::decode(&serde_json::to_vec(&wire)?, [7; 32]).is_err(), "{key}");
    }
    assert!(Request::decode(&fixture().encode()?, [8; 32]).is_err());
    assert!(Request::decode(b"{}", [7; 32]).is_err());
    Ok(())
}

#[test]
fn source_slots_must_be_distinct_nonzero_non_pseudo_and_bounded() -> TestResult {
    for pins in [vec![], vec![0], vec![u64::MAX], vec![12], vec![16, 16], vec![16; MAX_PINS + 1]] {
        let mut request = fixture();
        request.pins = pins;
        assert!(request.encode().is_err());
    }
    let mut request = fixture();
    request.entry = Entry::Trial { run: 4, snapshot: 4 };
    assert!(request.encode().is_err());
    let mut request = fixture();
    request.executable.push('\0');
    assert!(request.encode().is_err());
    let mut request = fixture();
    request.directory = "a".repeat(16385);
    assert!(request.encode().is_err());
    assert!(Request::decode(&vec![b' '; MAX_BYTES + 1], [7; 32]).is_err());
    let mut request = fixture();
    request.pins = vec![16; MAX_PINS + 1];
    assert!(Request::decode(&serde_json::to_vec(&request)?, [7; 32]).is_err());
    Ok(())
}

#[test]
fn duplicate_fields_and_unknown_entry_modes_are_rejected() -> TestResult {
    let text = String::from_utf8(fixture().encode()?)?;
    let duplicate = text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert!(Request::decode(duplicate.as_bytes(), [7; 32]).is_err());
    for entry in [
        serde_json::json!({"kind":"shell","selection":12}),
        serde_json::json!({"kind":"installed","selection":12,"extra":1}),
    ] {
        let mut value = serde_json::to_value(fixture())?;
        value["entry"] = entry;
        assert!(Request::decode(&serde_json::to_vec(&value)?, [7; 32]).is_err());
    }
    Ok(())
}

#[test]
fn replies_require_the_exact_original_identity_and_observed_disposition() -> TestResult {
    for outcome in
        [CreationOutcome::NotCreated, CreationOutcome::Created, CreationOutcome::Indeterminate]
    {
        let bytes = result([7; 32], outcome)?;
        assert_eq!(response(&bytes, [7; 32])?, outcome);
        assert!(response(&bytes, [8; 32]).is_err());
    }
    for disposition in ["healthy", "accepted", "cancelled", "timed_out"] {
        let wire = serde_json::json!({"version":1,"id":vec![7;32],"disposition":disposition});
        assert!(response(&serde_json::to_vec(&wire)?, [7; 32]).is_err());
    }
    assert!(response(&vec![b' '; 1025], [7; 32]).is_err());
    assert!(result([0; 32], CreationOutcome::NotCreated).is_err());
    Ok(())
}
