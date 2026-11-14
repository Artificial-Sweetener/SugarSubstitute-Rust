// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The desktop child keeps the user's own profile; confined storage stays with the processes
//! that need it.
#![cfg(windows)]
use std::error::Error;
use sugarsubstitute_supervision::LaunchEnvironment;
type TestResult = Result<(), Box<dyn Error>>;

const PROFILE: [&str; 6] = [
    r"APPDATA=C:\Users\person\AppData\Roaming",
    r"HOME=C:\Users\person",
    r"LOCALAPPDATA=C:\Users\person\AppData\Local",
    r"TEMP=C:\Users\person\AppData\Local\Temp",
    r"TMP=C:\Users\person\AppData\Local\Temp",
    r"USERPROFILE=C:\Users\person",
];

fn snapshot(lines: &[&str]) -> Vec<u16> {
    let mut result = Vec::new();
    for line in lines {
        result.extend(line.encode_utf16());
        result.push(0);
    }
    result.push(0);
    result
}
fn lines(environment: &LaunchEnvironment) -> Vec<String> {
    environment
        .as_utf16()
        .split(|unit| *unit == 0)
        .filter(|line| !line.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// The desktop child keeps the user's profile folders, so Windows dialogs and known folders
/// resolve to the user's own; inherited WebView2 overrides are removed so the browser profile
/// stays where the desktop names it.
#[test]
fn the_desktop_child_keeps_the_users_profile_and_drops_browser_overrides() -> TestResult {
    let mut captured = PROFILE.to_vec();
    captured.push("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu");
    captured.push(r"WEBVIEW2_USER_DATA_FOLDER=C:\Elsewhere");
    let desktop = LaunchEnvironment::for_desktop(&snapshot(&captured))?;
    assert_eq!(lines(&desktop), PROFILE);
    Ok(())
}

/// A shipped build drops even a lone remote-debugging port, so nothing outside can drive the page.
#[cfg(not(feature = "page-automation-qualification"))]
#[test]
fn a_shipped_desktop_child_drops_a_debugging_port() -> TestResult {
    let mut captured = PROFILE.to_vec();
    captured.push("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333");
    let desktop = LaunchEnvironment::for_desktop(&snapshot(&captured))?;
    assert_eq!(lines(&desktop), PROFILE);
    Ok(())
}

/// A qualification build lets a lone remote-debugging port reach the page so a proof can drive
/// and read it; the browser-profile override is still removed.
#[cfg(feature = "page-automation-qualification")]
#[test]
fn a_qualification_desktop_child_keeps_a_lone_debugging_port() -> TestResult {
    let port = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333";
    let mut captured = PROFILE.to_vec();
    captured.push(port);
    captured.push(r"WEBVIEW2_USER_DATA_FOLDER=C:\Elsewhere");
    let desktop = LaunchEnvironment::for_desktop(&snapshot(&captured))?;
    let mut expected = PROFILE.to_vec();
    expected.push(port);
    assert_eq!(lines(&desktop), expected);
    Ok(())
}

/// From the same capture, a confined process still receives its storage folder for every home,
/// settings and temporary variable.
#[test]
fn confined_children_still_receive_their_storage_folder() -> TestResult {
    let storage = std::path::Path::new(r"E:\install\user\comfy-data");
    let confined = LaunchEnvironment::for_local_storage(&snapshot(&PROFILE), storage)?;
    let lines = lines(&confined);
    for name in ["APPDATA", "HOME", "LOCALAPPDATA", "TEMP", "TMP", "USERPROFILE"] {
        let expected = format!(r"{name}=E:\install\user\comfy-data");
        assert!(lines.contains(&expected), "{name}: {lines:?}");
    }
    Ok(())
}
