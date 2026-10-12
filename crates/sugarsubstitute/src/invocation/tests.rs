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

/// Pipe roles use the actual parser while reserving all native effects for execution.
#[cfg(target_os = "windows")]
#[test]
fn native_pipe_roles_require_exact_inputs() -> Result<(), Box<dyn Error>> {
    let path = r"D:\evidence\pipe.json";
    assert!(matches!(Invocation::parse_arguments(
        ["--internal-native-pipe-fixture", "--receipt", path].map(OsString::from)
    )?, Invocation::NativePipeFixture(actual) if actual == std::path::Path::new(path)));
    for case in ["valid", "wrong-capability", "foreign-generation"] {
        assert!(matches!(
            Invocation::parse_arguments(
                [
                    "--internal-native-pipe-child",
                    "--section",
                    "42",
                    "--deadline-qpc",
                    "200",
                    "--qpc-frequency",
                    "100",
                    "--case",
                    case,
                ]
                .map(OsString::from)
            )?,
            Invocation::NativePipeChild(_)
        ));
    }
    Ok(())
}

/// Mixed, reordered and non-Unicode role inputs fail before native adoption.
#[cfg(target_os = "windows")]
#[test]
fn native_pipe_roles_reject_ambiguous_invocations() {
    use std::os::windows::ffi::OsStringExt;
    for arguments in [
        vec!["--internal-native-pipe-fixture"],
        vec!["--internal-native-pipe-fixture", "--receipt", "relative.json"],
        vec!["--internal-native-pipe-fixture", "--receipt", "D:relative.json"],
        vec!["--internal-native-pipe-fixture", "--receipt", r"D:\pipe.json", "--light"],
        vec!["--light", "--internal-native-pipe-fixture", "--receipt", r"D:\pipe.json"],
        vec!["--internal-native-pipe-child", "--section", "42", "--case", "valid"],
        vec![
            "--internal-native-pipe-child",
            "--section",
            "42",
            "--deadline-qpc",
            "200",
            "--qpc-frequency",
            "100",
            "--case",
            "valid",
            "--receipt",
            r"D:\pipe.json",
        ],
    ] {
        assert!(Invocation::parse_arguments(arguments.into_iter().map(OsString::from)).is_err());
    }
    assert!(
        Invocation::parse_arguments([
            OsString::from("--internal-native-pipe-fixture"),
            OsString::from_wide(&[0xd800]),
            OsString::from(r"D:\pipe.json"),
        ])
        .is_err()
    );
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
        Invocation::NativeCaptureFixture(_) => {
            return Err("Presentation options selected a capture fixture".into());
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
        Invocation::NativePipeFixture(_)
        | Invocation::NativePipeChild(_)
        | Invocation::NativeOutputFixture(_)
        | Invocation::NativeOutputChild(_) => {
            return Err("Presentation options selected a lifecycle pipe fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeHeldRegistryFixture(_) => {
            return Err("Presentation options selected a held registry fixture".into());
        }
        #[cfg(target_os = "windows")]
        Invocation::NativeClaimFixture(_)
        | Invocation::NativeClaimChild(_)
        | Invocation::NativeForwardingRoot(_)
        | Invocation::NativeDescendantFixture(_) => {
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

/// Both fixed descendant roles reject malformed or mixed arguments before effects.
#[cfg(target_os = "windows")]
#[test]
fn native_descendant_fixture_dispatch_preserves_fixed_roles() -> Result<(), Box<dyn Error>> {
    let parsed = Invocation::parse_arguments(
        ["--internal-native-descendant-fixture", "--receipt", "D:\\evidence\\descendant.json"]
            .map(OsString::from),
    )?;
    assert!(matches!(parsed,Invocation::NativeDescendantFixture(path)
        if path==std::path::Path::new("D:\\evidence\\descendant.json")));
    let parsed = Invocation::parse_arguments(
        [
            "--internal-native-forwarding-root",
            "--ready",
            "11",
            "--release",
            "12",
            "--challenge",
            "13",
            "--response",
            "14",
            "--deadline-qpc",
            "1000",
            "--qpc-frequency",
            "1000",
            "--correlation",
            "1",
            "--exit-code",
            "0",
        ]
        .map(OsString::from),
    )?;
    assert!(matches!(parsed,Invocation::NativeForwardingRoot(options)
        if options.deadline.ticks()==1000 && options.correlation.get()==1 && options.exit_code==0));
    for args in [
        vec!["--internal-native-descendant-fixture"],
        vec!["--internal-native-descendant-fixture", "--receipt", "relative.json"],
        vec!["--internal-native-descendant-fixture", "--receipt", "D:\\receipt.json", "--light"],
        vec!["--internal-native-forwarding-root"],
        vec!["--internal-native-forwarding-root", "--internal-native-claim-child"],
        vec!["--light", "--internal-native-descendant-fixture", "--receipt", "D:\\receipt.json"],
    ] {
        assert!(Invocation::parse_arguments(args.into_iter().map(OsString::from)).is_err());
    }
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

/// Capture dispatch validates the ordered role before native or filesystem effects.
#[cfg(target_os = "windows")]
#[test]
fn native_capture_fixture_dispatch_requires_exact_role_and_complete_inputs()
-> Result<(), Box<dyn Error>> {
    let arguments: Vec<OsString> = [
        "--internal-native-capture-fixture",
        "--client",
        r"E:\bundle\client.dll",
        "--handler",
        r"E:\bundle\handler.exe",
        "--capture-root",
        r"D:\proof\protected",
        "--client-sha256",
        &"01".repeat(32),
        "--handler-sha256",
        &"02".repeat(32),
        "--namespace",
        &"03".repeat(32),
        "--epoch",
        &"04".repeat(16),
        "--build",
        "fixture-build",
        "--receipt",
        r"D:\proof\receipt.json",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    assert!(
        matches!(Invocation::parse_arguments(arguments.clone())?, Invocation::NativeCaptureFixture(options)
        if options.receipt == std::path::Path::new(r"D:\proof\receipt.json"))
    );
    let mut extra = arguments.clone();
    extra.push("--light".into());
    assert!(Invocation::parse_arguments(extra).is_err());
    let mut mixed = arguments.clone();
    mixed.insert(0, "--light".into());
    assert!(Invocation::parse_arguments(mixed).is_err());
    assert!(Invocation::parse_arguments(arguments.into_iter().take(1)).is_err());
    Ok(())
}
