// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real executable dispatch before native or filesystem effects.
use super::*;
/// Internal claim roles dispatch through the real parser before any native effects.
#[cfg(target_os = "windows")]
#[test]
fn native_claim_fixture_dispatch_requires_exact_ordered_headless_inputs()
-> Result<(), Box<dyn Error>> {
    let root = r"D:\SugarSubstitute-Rust\artifacts\m1\fresh";
    let receipt = r"D:\SugarSubstitute-Rust\artifacts\m1\claim.json";
    let parsed = Invocation::parse_arguments(
        ["--internal-native-claim-fixture", "--workspace", root, "--receipt", receipt]
            .map(OsString::from),
    )?;
    assert!(
        matches!(parsed,Invocation::NativeClaimFixture(options) if options.workspace==std::path::Path::new(root) && options.receipt==std::path::Path::new(receipt))
    );
    let arguments = [
        "--internal-native-claim-child",
        "--role",
        "coordinator-contender",
        "--installation",
        root,
        "--backend",
        root,
        "--receipt",
        receipt,
        "--namespace",
        &"ab".repeat(32),
        "--epoch",
        &"cd".repeat(16),
        "--ready",
        "11",
        "--release",
        "12",
        "--challenge",
        "13",
        "--response",
        "14",
        "--deadline-qpc",
        "200",
        "--qpc-frequency",
        "100",
        "--correlation",
        "1",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    assert!(
        matches!(Invocation::parse_arguments(arguments)?,Invocation::NativeClaimChild(options) if options.deadline.ticks()==200 && options.correlation.get()==1)
    );
    assert!(
        Invocation::parse_arguments(
            ["--internal-native-claim-fixture", "--receipt", receipt, "--workspace", root]
                .map(OsString::from)
        )
        .is_err()
    );
    assert!(
        Invocation::parse_arguments(
            [
                "--internal-native-claim-fixture",
                "--workspace",
                root,
                "--receipt",
                receipt,
                "--light"
            ]
            .map(OsString::from)
        )
        .is_err()
    );
    Ok(())
}

/// Preserve the native proof's combined presentation and receipt options.
#[test]
fn demonstration_options_keep_their_invocation_role() -> Result<(), Box<dyn Error>> {
    let invocation = Invocation::parse_arguments(
        [
            "--light",
            "--collapsed",
            "--reduced-motion",
            "--locale",
            "ja",
            "--duration-ms",
            "750",
            "--timing-audit",
            "--receipt",
            "receipt.json",
        ]
        .map(OsString::from),
    )?;
    let options = match invocation {
        Invocation::Demo(options) => options,
        #[cfg(target_os = "windows")]
        Invocation::NativeExitFixture(_) => {
            return Err("Presentation options selected an exit fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeRegistryFixture(_) => {
            return Err("Presentation options selected a registry fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeHeldChild(_) => {
            return Err("Presentation options selected a held child fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeHeldRegistryFixture(_) => {
            return Err("Presentation options selected a held registry fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeClaimFixture(_) | Invocation::NativeClaimChild(_) => {
            return Err("Presentation options selected a claim fixture".into());
        }
    };
    assert_eq!(options.appearance.theme, Theme::Light);
    assert!(!options.appearance.expanded);
    assert!(options.reduced_motion);
    assert_eq!(options.locale, "ja");
    assert_eq!(options.duration, Some(Duration::from_millis(750)));
    assert!(options.timing_audit);
    assert_eq!(options.receipt, Some(PathBuf::from("receipt.json")));
    Ok(())
}

/// Exit 259 is a genuine signaled exit despite sharing STILL_ACTIVE's numeric value.
#[cfg(target_os = "windows")]
#[test]
fn native_exit_fixture_preserves_all_supported_windows_codes() -> Result<(), Box<dyn Error>> {
    for (argument, expected) in [("0", 0), ("37", 37), ("259", 259)] {
        let invocation = Invocation::parse_arguments(
            ["--internal-native-exit-fixture", argument].map(OsString::from),
        )?;
        assert!(matches!(invocation, Invocation::NativeExitFixture(code) if code == expected));
    }
    Ok(())
}

/// Fixture selection rejects missing, extra, unsupported and non-Unicode input.
#[cfg(target_os = "windows")]
#[test]
fn native_exit_fixture_rejects_ambiguous_invocations() {
    use std::os::windows::ffi::OsStringExt;

    for arguments in [
        vec!["--internal-native-exit-fixture"],
        vec!["--internal-native-exit-fixture", "1"],
        vec!["--internal-native-exit-fixture", "-1"],
        vec!["--internal-native-exit-fixture", "0259"],
        vec!["--internal-native-exit-fixture", "37", "--light"],
        vec!["--light", "--internal-native-exit-fixture", "0"],
    ] {
        assert!(Invocation::parse_arguments(arguments.into_iter().map(OsString::from)).is_err());
    }
    assert!(
        Invocation::parse_arguments([
            OsString::from("--internal-native-exit-fixture"),
            OsString::from_wide(&[0xd800]),
        ])
        .is_err()
    );
}

/// Registry qualification has a single explicit role and an absolute output path.
#[cfg(target_os = "windows")]
#[test]
fn native_registry_fixture_accepts_only_its_exact_role() -> Result<(), Box<dyn Error>> {
    let result = Invocation::parse_arguments(
        ["--internal-native-registry-fixture", "--receipt", "D:\\evidence\\receipt.json"]
            .map(OsString::from),
    )?;
    assert!(
        matches!(result, Invocation::NativeRegistryFixture(path) if path == std::path::Path::new("D:\\evidence\\receipt.json"))
    );
    Ok(())
}

/// Relative destinations, mixed roles and non-Unicode flags fail before effects.
#[cfg(target_os = "windows")]
#[test]
fn native_registry_fixture_rejects_ambiguous_or_missing_arguments() {
    use std::os::windows::ffi::OsStringExt;
    for arguments in [
        vec!["--internal-native-registry-fixture"],
        vec!["--internal-native-registry-fixture", "--receipt"],
        vec!["--internal-native-registry-fixture", "--receipt", "relative.json"],
        vec!["--internal-native-registry-fixture", "--receipt", "D:relative.json"],
        vec!["--internal-native-registry-fixture", "--receipt", "D:\\receipt.json", "--light"],
        vec!["--light", "--internal-native-registry-fixture", "--receipt", "D:\\receipt.json"],
        vec!["--internal-native-registry-fixture", "--internal-native-exit-fixture", "0"],
    ] {
        assert!(Invocation::parse_arguments(arguments.into_iter().map(OsString::from)).is_err());
    }
    assert!(
        Invocation::parse_arguments([
            OsString::from("--internal-native-registry-fixture"),
            OsString::from_wide(&[0xd800]),
            OsString::from("D:\\receipt.json"),
        ])
        .is_err()
    );
    assert!(Invocation::parse_arguments([OsString::from_wide(&[0xd800])]).is_err());
}

/// Timing evidence requires a native receipt destination and cannot silently disappear offline.
#[test]
fn timing_qualification_rejects_missing_receipt_and_offline_export() -> Result<(), Box<dyn Error>> {
    let arguments =
        |values: &[&str]| values.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    assert!(DemoOptions::parse_arguments(arguments(&["--timing-audit"])).is_err());
    assert!(
        DemoOptions::parse_arguments(arguments(&[
            "--timing-audit",
            "--receipt",
            "receipt.json",
            "--scenarios",
            "inputs.json",
            "--output",
            "frames",
        ]))
        .is_err()
    );
    let options =
        DemoOptions::parse_arguments(arguments(&["--timing-audit", "--receipt", "receipt.json"]))?;
    assert!(options.timing_audit);
    assert_eq!(options.receipt, Some(PathBuf::from("receipt.json")));
    assert!(options.frame_audit.is_none());
    Ok(())
}
