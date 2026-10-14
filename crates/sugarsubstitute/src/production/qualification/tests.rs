// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise inert lexical admission; these tests never construct a capture seal or execute a trap.
use super::*;
type TestResult = Result<(), Box<dyn Error>>;
fn arguments(values: &[&str]) -> std::iter::Peekable<std::vec::IntoIter<OsString>> {
    values.iter().map(OsString::from).collect::<Vec<_>>().into_iter().peekable()
}
#[test]
fn ordinary_production_and_legacy_grammar_are_unchanged() -> TestResult {
    for values in
        [&["--production", "--installation-root", r"E:\install"][..], &["--locale", "ja"][..]]
    {
        let mut source = arguments(values);
        let intent = QualificationIntent::root(&mut source)?;
        assert_eq!(
            source.collect::<Vec<_>>(),
            values.iter().map(OsString::from).collect::<Vec<_>>()
        );
        let mut child = Vec::new();
        intent.append(&mut child).map_err(|cause| cause as Box<dyn Error>)?;
        assert!(child.is_empty());
    }
    Ok(())
}
#[cfg(not(feature = "k1-native-qualification"))]
#[test]
fn ordinary_build_rejects_root_and_desktop_qualification() {
    for flag in ["--k1-native-fault-once", "--k1-native-preview-once"] {
        assert!(QualificationIntent::root(&mut arguments(&[flag, "--production"])).is_err());
    }
    for flag in ["--internal-k1-native-fault", "--internal-k1-native-preview"] {
        assert!(QualificationIntent::child(&mut [OsString::from(flag)].into_iter()).is_err());
    }
}
#[cfg(feature = "k1-native-qualification")]
#[test]
fn fault_intent_is_lexical_once_and_requires_original_production_grammar() -> TestResult {
    let mut root = arguments(&[
        "--k1-native-fault-once",
        "--production",
        "--installation-root",
        r"E:\install",
    ]);
    let intent = QualificationIntent::root(&mut root)?;
    assert!(intent.fault());
    assert_eq!(root.peek(), Some(&OsString::from("--production")));
    let mut child = Vec::new();
    intent.append(&mut child).map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(child, vec!["--internal-k1-native-fault".encode_utf16().collect::<Vec<_>>()]);
    let inherited = QualificationIntent::child(
        &mut [OsString::from("--internal-k1-native-fault")].into_iter(),
    )?;
    assert!(inherited.fault());
    for values in [
        &["--k1-native-fault-once"][..],
        &["--k1-native-fault-once", "--internal-production-reporter"][..],
        &["--k1-native-fault-once", "--k1-native-fault-once", "--production"][..],
    ] {
        assert!(QualificationIntent::root(&mut arguments(values)).is_err());
    }
    assert!(
        QualificationIntent::child(
            &mut ["--internal-k1-native-fault", "--internal-k1-native-fault"]
                .map(OsString::from)
                .into_iter()
        )
        .is_err()
    );
    Ok(())
}
#[cfg(feature = "k1-native-qualification")]
#[test]
fn preview_preserves_unicode_original_counter_and_owned_d_destination_once() -> TestResult {
    let mut source = arguments(&[
        "--k1-native-preview-once",
        "--preview-until-qpc",
        "9000",
        "--preview-qpc-frequency",
        "1000",
        "--preview-destination",
        "D:\\captures\\日本語.png",
        "--production",
    ]);
    let mut intent = QualificationIntent::root(&mut source)?;
    let preview = intent.take_preview().ok_or("Original preview missing")?;
    assert_eq!(preview.deadline().ticks(), 9000);
    assert_eq!(preview.deadline().frequency(), 1000);
    assert_eq!(preview.destination(), std::path::Path::new("D:\\captures\\日本語.png"));
    assert!(intent.take_preview().is_none());
    assert!(!intent.fault());
    Ok(())
}
#[cfg(feature = "k1-native-qualification")]
#[test]
fn preview_rejects_foreign_drive_unc_ads_relative_and_malformed_cutoff() {
    for path in [
        r"C:\captures\proof.png",
        r"\\server\share\proof.png",
        r"D:\captures\proof.png:stream",
        "relative.png",
        "D:\\captures\\..\\proof.png",
    ] {
        let mut source = arguments(&[
            "--k1-native-preview-once",
            "--preview-until-qpc",
            "9000",
            "--preview-qpc-frequency",
            "1000",
            "--preview-destination",
            path,
            "--production",
        ]);
        assert!(QualificationIntent::root(&mut source).is_err());
    }
    for (ticks, frequency) in [("0", "1000"), ("-1", "1000"), ("9000", "0"), ("9000", "-1")] {
        let mut source = arguments(&[
            "--k1-native-preview-once",
            "--preview-until-qpc",
            ticks,
            "--preview-qpc-frequency",
            frequency,
            "--preview-destination",
            r"D:\captures\proof.png",
            "--production",
        ]);
        assert!(QualificationIntent::root(&mut source).is_err());
    }
}

#[cfg(feature = "k1-native-qualification")]
#[test]
fn preview_rejects_short_destination_with_excess_retained_capacity() {
    let mut destination = OsString::with_capacity(8192);
    destination.push(r"D:\captures\proof.png");
    let values = [
        OsString::from("--k1-native-preview-once"),
        OsString::from("--preview-until-qpc"),
        OsString::from("9000"),
        OsString::from("--preview-qpc-frequency"),
        OsString::from("1000"),
        OsString::from("--preview-destination"),
        destination,
        OsString::from("--production"),
    ];
    assert!(QualificationIntent::root(&mut values.into_iter().peekable()).is_err());
}
