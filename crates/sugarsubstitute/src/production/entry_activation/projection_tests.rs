// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check exact Windows units and authoritative lexical validation in the funded projection.
use super::*;
use std::os::windows::ffi::OsStrExt;
fn bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}
#[test]
fn projection_preserves_unpaired_profile_path_units() -> Result<(), Box<dyn Error + Send + Sync>> {
    let original = [69, 58, 92, 0xd800, 97, 0xdc01];
    let raw = bytes(&original);
    let projected = path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), original);
    Ok(())
}
#[test]
fn projection_rejects_relative_and_parent_traversal_without_rewriting()
-> Result<(), Box<dyn Error + Send + Sync>> {
    for value in ["profile", r"E:\parent\..\profile"] {
        let raw = bytes(&value.encode_utf16().collect::<Vec<_>>());
        assert!(path(EntryWindowsPathView::from_le_bytes(&raw)?).is_err());
    }
    Ok(())
}
#[test]
fn projection_path_exact_unit_bound_survives_owned_conversion()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut units =
        vec![u16::from(b'x'); sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_PATH_UNITS];
    units[..3].copy_from_slice(&[69, 58, 92]);
    let raw = bytes(&units);
    let projected = path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    Ok(())
}

/// Borrow valid wire shape only; this fixture grants no authenticated source or process privilege.
fn managed_view<'a>(
    root: &'a [u8],
    interpreter: &'a [u8],
    arguments: &'a [u8],
    count: u8,
) -> Result<EntryLaunchRequestView<'a>, Box<dyn Error + Send + Sync>> {
    use sugarsubstitute_contracts::{
        configuration::{Presentation, Theme},
        coordinator_entry::{EntryArgumentsView, EntryEnvironmentView, EntryFrontendView},
    };
    let root = EntryWindowsPathView::from_le_bytes(root)?;
    let interpreter = EntryWindowsPathView::from_le_bytes(interpreter)?;
    Ok(EntryLaunchRequestView::new(
        EntryConnectionView::Managed {
            root,
            interpreter,
            arguments: EntryArgumentsView::from_encoded(arguments, count)?,
            port: std::num::NonZeroU16::new(18189).ok_or("Invalid port")?,
        },
        EntryFrontendView::new(root, root, root, root),
        root,
        "Profile_1",
        Presentation::new("en", Theme::Dark, false)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?)
}

/// Empty schema-one extras still yield the complete Python/Comfy command before identity resolution.
#[test]
fn parent_projection_composes_complete_managed_command_from_empty_extras()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let root = bytes(&r"E:\comfyui".encode_utf16().collect::<Vec<_>>());
    let interpreter =
        bytes(&r"E:\comfyui\venv\Scripts\python.exe".encode_utf16().collect::<Vec<_>>());
    let original = managed_view(&root, &interpreter, &[], 0)?;
    let projected = fields(original)?;
    let BackendTarget::Managed(launch) = projected.target else {
        return Err("Managed target changed mode".into());
    };
    assert_eq!(
        launch.arguments(),
        [
            "-u",
            r"E:\comfyui\main.py",
            "--listen",
            "127.0.0.1",
            "--port",
            "18189",
            "--disable-auto-launch"
        ]
    );
    assert_eq!(launch.root(), std::path::Path::new(r"E:\comfyui"));
    assert_eq!(launch.interpreter(), std::path::Path::new(r"E:\comfyui\venv\Scripts\python.exe"));
    assert!(projected.backend_environment.is_some());
    let EntryConnectionView::Managed { arguments, .. } = original.connection() else {
        return Err("Original mode changed".into());
    };
    assert!(arguments.is_empty());
    Ok(())
}

/// Managed prefix entries remain included in the sole builder's authoritative argument count.
#[test]
fn parent_projection_rejects_effective_command_overflow_without_changing_extra_section()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let root = bytes(&r"E:\comfyui".encode_utf16().collect::<Vec<_>>());
    let interpreter = bytes(&r"E:\python.exe".encode_utf16().collect::<Vec<_>>());
    let mut arguments = Vec::new();
    for _ in 0..58 {
        arguments.extend_from_slice(&5u32.to_le_bytes());
        arguments.extend_from_slice(b"--cpu");
    }
    let original = managed_view(&root, &interpreter, &arguments, 58)?;
    let cause = fields(original).err().ok_or("Complete command limit bypassed")?;
    assert!(matches!(
        cause.downcast_ref::<sugarsubstitute_comfy::launch::ManagedLaunchError>(),
        Some(sugarsubstitute_comfy::launch::ManagedLaunchError::Configuration(
            sugarsubstitute_contracts::configuration::ConfigurationError::TooManyArguments
        ))
    ));
    let EntryConnectionView::Managed { arguments: retained, .. } = original.connection() else {
        return Err("Original mode changed".into());
    };
    assert_eq!(retained.iter().count(), 58);
    assert!(retained.iter().all(|value| value == "--cpu"));
    Ok(())
}
