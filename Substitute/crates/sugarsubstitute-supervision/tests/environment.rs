// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise explicit Windows environment preparation without ambient state or children.
#![cfg(windows)]
use std::{collections::TryReserveError, error::Error};
use sugarsubstitute_supervision::{
    EnvironmentOverride as Change, LaunchEnvironment, LaunchEnvironmentError as Failure,
};
type TestResult = Result<(), Box<dyn Error>>;
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn snapshot(lines: &[&str]) -> Vec<u16> {
    let mut result = Vec::new();
    for line in lines {
        result.extend(wide(line));
        result.push(0);
    }
    result.push(0);
    if lines.is_empty() {
        result.push(0);
    }
    result
}

#[test]
fn empty_and_first_variable_have_exact_double_nul() -> TestResult {
    let empty = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
    assert_eq!(empty.as_utf16(), [0, 0]);
    let name = wide("First");
    let value = wide("");
    let first = LaunchEnvironment::from_snapshot_utf16(
        &[0, 0],
        &[Change::Set { name: &name, value: &value }],
    )?;
    assert_eq!(first.into_utf16(), snapshot(&["First="]));
    Ok(())
}
#[test]
fn sorting_replacement_and_removal_are_native_case_insensitive() -> TestResult {
    let name = wide("pAtH");
    let value = wide("new=literal & %PATH%");
    let remove = wide("z");
    let absent = wide("Absent");
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &snapshot(&["z=last", "PATH=old", "a=first"]),
        &[
            Change::Set { name: &name, value: &value },
            Change::Remove { name: &remove },
            Change::Remove { name: &absent },
        ],
    )?;
    assert_eq!(prepared.as_utf16(), snapshot(&["a=first", "pAtH=new=literal & %PATH%"]));
    Ok(())
}
#[test]
fn unicode_ordinal_matching_and_order_are_not_ascii_folding() -> TestResult {
    let name = wide("éCLAIR");
    let value = wide("東京😀e\u{301}");
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &snapshot(&["Éclair=old", "z=last", "A=first"]),
        &[Change::Set { name: &name, value: &value }],
    )?;
    assert_eq!(prepared.as_utf16(), snapshot(&["A=first", "z=last", "éCLAIR=東京😀e\u{301}"]));
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(&snapshot(&["É=one", "é=two"]), &[]),
        Err(Failure::DuplicateSnapshot)
    ));
    Ok(())
}
#[test]
fn lone_surrogates_in_names_and_values_are_preserved() -> TestResult {
    let name = [0xd800];
    let value = [0xdfff, 61, 0xd801];
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &[0, 0],
        &[Change::Set { name: &name, value: &value }],
    )?;
    assert_eq!(prepared.as_utf16(), [0xd800, 61, 0xdfff, 61, 0xd801, 0, 0]);
    let again = LaunchEnvironment::from_snapshot_utf16(prepared.as_utf16(), &[])?;
    assert_eq!(again.as_utf16(), prepared.as_utf16());
    Ok(())
}
#[test]
fn snapshot_hidden_drive_entries_are_preserved_and_sorted() -> TestResult {
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &snapshot(&["Path=p", "=Z:=Z:\\work", "=C:=C:\\folder"]),
        &[],
    )?;
    assert_eq!(prepared.as_utf16(), snapshot(&["=C:=C:\\folder", "=Z:=Z:\\work", "Path=p"]));
    for invalid in ["=", "=Z", "=ZZ:", "=1:", "Name=bad"] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(
                &[0, 0],
                &[Change::Set { name: &wide(invalid), value: &[] }]
            ),
            Err(Failure::InvalidName)
        ));
    }
    for invalid in ["==bad", "NoSeparator"] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(&snapshot(&[invalid]), &[]),
            Err(Failure::InvalidName)
        ));
    }
    Ok(())
}
#[test]
fn inherited_hidden_shell_metadata_survives_child_environment_preparation() -> TestResult {
    let original = snapshot(&["Path=p", "=::=::\\", "=ExitCode=00000000", "=1:=opaque"]);
    let prepared = LaunchEnvironment::from_snapshot_utf16(&original, &[])?;
    assert_eq!(
        prepared.as_utf16(),
        snapshot(&["=1:=opaque", "=::=::\\", "=ExitCode=00000000", "Path=p"])
    );
    assert_eq!(
        LaunchEnvironment::from_snapshot_utf16(prepared.as_utf16(), &[])?.as_utf16(),
        prepared.as_utf16()
    );
    for hidden in ["=::", "=ExitCode"] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(
                &[0, 0],
                &[Change::Set { name: &wide(hidden), value: &wide("value") }]
            ),
            Err(Failure::InvalidName)
        ));
    }
    Ok(())
}
#[test]
fn project_private_prefixes_are_stripped_and_cannot_be_inserted() -> TestResult {
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &snapshot(&[
            "sUgAr_sUbStItUtE_SECRET=one",
            "sugarsubstitute_token=two",
            "Substitute_RUN=three",
            "SUGAR_SUBSTITUTE=ordinary",
            "PUBLIC=keep",
        ]),
        &[Change::Remove { name: &wide("SUBSTITUTE_OLD") }],
    )?;
    assert_eq!(prepared.as_utf16(), snapshot(&["PUBLIC=keep", "SUGAR_SUBSTITUTE=ordinary"]));
    for private in ["SUGAR_SUBSTITUTE_", "sugarsubstitute_SECRET", "substitute_Run"] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(
                &[0, 0],
                &[Change::Set { name: &wide(private), value: &wide("secret") }]
            ),
            Err(Failure::PrivateVariable)
        ));
    }
    Ok(())
}
#[test]
fn duplicate_snapshot_and_override_names_reject_including_removals_and_private_entries() {
    for lines in
        [vec!["PATH=a", "Path=b"], vec!["=C:=a", "=c:=b"], vec!["SUBSTITUTE_X=a", "substitute_x=b"]]
    {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(&snapshot(&lines), &[]),
            Err(Failure::DuplicateSnapshot)
        ));
    }
    let upper = wide("PATH");
    let lower = wide("path");
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(
            &[0, 0],
            &[Change::Set { name: &upper, value: &[] }, Change::Remove { name: &lower },]
        ),
        Err(Failure::DuplicateOverride)
    ));
}
#[test]
fn malformed_termination_and_nul_names_or_values_reject() {
    for invalid in [
        vec![],
        vec![0],
        vec![0, 0, 0],
        wide("A=b"),
        vec![65, 61, 98, 0],
        vec![65, 61, 98, 0, 0, 0],
        snapshot(&["A=b", "", "B=c"]),
    ] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(&invalid, &[]),
            Err(Failure::InvalidSnapshot)
        ));
    }
    for name in [vec![], wide("=Hidden"), vec![65, 0, 66], wide("A=B")] {
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[Change::Remove { name: &name }]),
            Err(Failure::InvalidName)
        ));
    }
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(
            &[0, 0],
            &[Change::Set { name: &wide("A"), value: &[98, 0, 99] }]
        ),
        Err(Failure::InteriorNul)
    ));
}
#[test]
fn source_and_override_limits_precede_content_validation() -> TestResult {
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(&vec![0; 32768], &[]),
        Err(Failure::SourceTooLong)
    ));
    let changes: Vec<_> = (0..65).map(|_| Change::Remove { name: &[] }).collect();
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(&[0, 0], &changes),
        Err(Failure::TooManyOverrides)
    ));
    let too_large = vec![0; 32768];
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[Change::Remove { name: &too_large }]),
        Err(Failure::SourceTooLong)
    ));
    let half = vec![0; 16384];
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(
            &[0, 0],
            &[Change::Set { name: &half, value: &half }]
        ),
        Err(Failure::SourceTooLong)
    ));
    let names: Vec<_> = (0..64).map(|index| wide(&format!("K{index:02}"))).collect();
    let changes: Vec<_> = names.iter().map(|name| Change::Set { name, value: &[] }).collect();
    assert!(LaunchEnvironment::from_snapshot_utf16(&[0, 0], &changes).is_ok());
    Ok(())
}
#[test]
fn exact_native_size_and_empty_values_are_accounted() -> TestResult {
    let name = wide("A");
    let exact = vec![97; 32763];
    let prepared = LaunchEnvironment::from_snapshot_utf16(
        &[0, 0],
        &[Change::Set { name: &name, value: &exact }],
    )?;
    assert_eq!(prepared.as_utf16().len(), 32767);
    assert_eq!(&prepared.as_utf16()[32765..], [0, 0]);
    assert_eq!(
        LaunchEnvironment::from_snapshot_utf16(prepared.as_utf16(), &[])?.as_utf16(),
        prepared.as_utf16()
    );
    let overflow = vec![97; 32764];
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(
            &[0, 0],
            &[Change::Set { name: &name, value: &overflow }]
        ),
        Err(Failure::BlockTooLong)
    ));
    Ok(())
}
#[test]
fn variable_limits_apply_to_final_state_after_removals() -> TestResult {
    let lines: Vec<_> = (0..1024).map(|index| format!("V{index:04}=")).collect();
    let borrowed: Vec<_> = lines.iter().map(String::as_str).collect();
    let original = snapshot(&borrowed);
    assert!(LaunchEnvironment::from_snapshot_utf16(&original, &[]).is_ok());
    let new = wide("A");
    let removed = wide("V1023");
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(
            &original,
            &[Change::Set { name: &new, value: &[] }]
        ),
        Err(Failure::TooManyVariables)
    ));
    let replacement = LaunchEnvironment::from_snapshot_utf16(
        &original,
        &[Change::Set { name: &new, value: &[] }, Change::Remove { name: &removed }],
    )?;
    assert!(replacement.as_utf16().starts_with(&wide("A=\0")));
    let mut excessive = original;
    excessive.pop();
    excessive.extend(wide("V1024=\0\0"));
    assert!(matches!(
        LaunchEnvironment::from_snapshot_utf16(&excessive, &[]),
        Err(Failure::TooManyVariables)
    ));
    Ok(())
}
#[test]
fn formatting_redacts_names_values_and_allocation_errors_keep_causes() -> TestResult {
    let name = wide("SECRET_NAME");
    let value = wide("SECRET_VALUE");
    let change = Change::Set { name: &name, value: &value };
    assert!(!format!("{change:?}").contains("SECRET"));
    let prepared = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[change])?;
    assert!(!format!("{prepared:?}").contains("SECRET"));
    let mut buffer: Vec<u16> = Vec::new();
    let Err(cause) = buffer.try_reserve(usize::MAX) else {
        return Err("expected reservation overflow".into());
    };
    let error = Failure::Allocation(cause);
    assert!(
        error.source().is_some_and(|source| source.downcast_ref::<TryReserveError>().is_some())
    );
    assert_eq!(format!("{error:?}"), format!("{error}"));
    Ok(())
}
