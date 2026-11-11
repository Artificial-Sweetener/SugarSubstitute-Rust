//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Prove SDK window association admission independently of Tauri and the user's desktop.
use sugarcubes_gimp_companion::launch::NativeLaunch;

#[test]
/// Retain standalone diagnostics without inventing a native parent or enabling private IO.
fn standalone_launch_has_no_native_authority() -> Result<(), String> {
    let launch = NativeLaunch::parse(None, None, Some("Unrelated inherited name"))?;
    assert!(!launch.native());
    assert!(launch.owner().is_none());
    assert_eq!(launch.title(), "SugarCubes — GIMP");
    assert!(NativeLaunch::parse(None, Some("42"), None).is_err());
    Ok(())
}

#[test]
/// Bind to exactly the SDK handle and launch image rather than an arbitrary foreground window.
fn native_launch_retains_the_explicit_invocation() -> Result<(), String> {
    let launch = NativeLaunch::parse(Some("1"), Some("38282442"), Some("Image.xcf\n\0"))?;
    assert!(launch.native());
    assert_eq!(launch.owner().map(|value| value.get()), Some(38282442));
    assert_eq!(launch.title(), "SugarCubes — Image.xcf — GIMP");
    assert!(
        NativeLaunch::parse(Some("1"), None, None)?
            .owner()
            .is_none()
    );
    Ok(())
}

#[test]
/// Reject malformed handles and unsupported admission values before any native window is created.
fn refuses_invalid_parent_values() {
    for owner in [
        "",
        "0",
        "-1",
        "+1",
        " 1",
        "1 ",
        "1.0",
        "18446744073709551616",
    ] {
        assert!(
            NativeLaunch::parse(Some("1"), Some(owner), None).is_err(),
            "{owner}"
        );
    }
    for native in ["0", "true", "", " 1"] {
        assert!(NativeLaunch::parse(Some(native), None, None).is_err());
    }
}

#[test]
/// Bound captions by characters without cutting a Unicode image name or admitting line breaks.
fn bounds_native_captions() -> Result<(), String> {
    let name = "é".repeat(300);
    assert_eq!(
        NativeLaunch::parse(Some("1"), None, Some(&name))?.title(),
        format!("SugarCubes — {} — GIMP", "é".repeat(256))
    );
    assert_eq!(
        NativeLaunch::parse(Some("1"), None, Some("\n\0"))?.title(),
        "SugarCubes — GIMP"
    );
    Ok(())
}
