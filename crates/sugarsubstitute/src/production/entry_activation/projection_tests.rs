// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check exact Windows units and authoritative lexical validation in the funded projection.
use super::*;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::coordinator_entry::{EntryConnectionView, EntryWindowsPathView};
fn bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}
#[test]
fn projection_preserves_unpaired_profile_path_units() -> Result<(), Box<dyn Error + Send + Sync>> {
    let original = [69, 58, 92, 0xd800, 97, 0xdc01];
    let raw = bytes(&original);
    let projected = fields::path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), original);
    Ok(())
}
#[test]
fn projection_rejects_relative_and_parent_traversal_without_rewriting()
-> Result<(), Box<dyn Error + Send + Sync>> {
    for value in ["profile", r"E:\parent\..\profile"] {
        let raw = bytes(&value.encode_utf16().collect::<Vec<_>>());
        assert!(fields::path(EntryWindowsPathView::from_le_bytes(&raw)?).is_err());
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
    let projected = fields::path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    Ok(())
}

/// Borrow valid wire shape only; this fixture grants no authenticated source or process privilege.
fn managed_view<'a>(
    root: &'a [u8],
    interpreter: &'a [u8],
    arguments: &'a [u8],
    count: u8,
    locale: &str,
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
        Presentation::new(locale, Theme::Dark, false)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?)
}

#[test]
fn originals_matching_data_never_bypasses_missing_native_source_or_installed_custody()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let root = bytes(&r"E:\comfyui".encode_utf16().collect::<Vec<_>>());
    let interpreter = bytes(&r"E:\python.exe".encode_utf16().collect::<Vec<_>>());
    let view = managed_view(&root, &interpreter, &[], 0, "en")?;
    assert!(validate_source(view, None, false).is_err());
    assert!(validate_source(view, None, true).is_ok());
    let referenced = view.with_originals(
        sugarsubstitute_contracts::coordinator_entry::EntryOriginalsReference::new([7; 32])?,
    );
    assert!(validate_source(referenced, None, false).is_err());
    assert!(validate_source(referenced, None, true).is_err());
    Ok(())
}

/// Empty schema-one extras still yield the complete Python/Comfy command before identity resolution.
#[test]
fn parent_projection_composes_complete_managed_command_from_empty_extras()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let root = bytes(&r"E:\comfyui".encode_utf16().collect::<Vec<_>>());
    let interpreter =
        bytes(&r"E:\comfyui\venv\Scripts\python.exe".encode_utf16().collect::<Vec<_>>());
    let original = managed_view(&root, &interpreter, &[], 0, "en")?;
    let capture = PathBuf::from(r"D:\SugarSubstitute-Rust\artifacts\inert-capture");
    let available = projection_data_bytes().ok_or("Joined projection partition underflow")?;
    let (claim, scope) = super::super::funding::tests::admitted(PROJECTION_BYTES, available)?;
    assert_eq!(PROJECTION_BYTES, 4 * 1024 * 1024);
    assert!(available < PROJECTION_BYTES);
    assert_eq!(claim.bytes(), available);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    let projected =
        fields::project(original, &capture, available, None, |_| Ok(BackendStorage::External))?;
    let BackendTarget::Managed(launch) = &projected.target else {
        return Err("Managed target changed mode".into());
    };
    assert_eq!(
        &launch.arguments()[3..],
        [
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
    let backend_environment = String::from_utf16(
        projected.backend_environment.as_ref().ok_or("Managed environment missing")?.as_utf16(),
    )?;
    for expected in [
        r"PATH=E:\comfyui\venv\Scripts;",
        "PYTHONIOENCODING=utf-8",
        "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER=1",
    ] {
        assert!(backend_environment.split('\0').any(|line| line == expected));
    }
    let desktop_environment = String::from_utf16(projected.desktop_environment.as_utf16())?;
    for expected in [r"TEMP=E:\", r"APPDATA=E:\", r"PIP_LOG=E:\pip.log"] {
        assert!(desktop_environment.split('\0').any(|line| line == expected));
    }
    assert_eq!(original.environment().units().collect::<Vec<_>>(), [0, 0]);
    let EntryConnectionView::Managed { arguments, .. } = original.connection() else {
        return Err("Original mode changed".into());
    };
    assert!(arguments.is_empty());
    // Initial and registered-restart projection use these same field/funding owners.
    // This exercises their compiled joined partition without fabricating a registered source.
    let mut input = ProjectionInput::without_request();
    input.capture_root = capture;
    input._charge = Some(claim);
    let installation =
        sugarsubstitute_contracts::installation::InstallationId::new("a".repeat(32))?;
    let (data, policy) = capacity::funding(&mut input, &projected, &installation)?;
    assert!(input._charge.is_none());
    assert!(policy.bytes() > 0 && policy.bytes() < available);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    drop(projected);
    drop(input);
    drop(policy);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    drop(data);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
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
    let original = managed_view(&root, &interpreter, &arguments, 58, "en")?;
    let cause = fields::project(
        original,
        &PathBuf::new(),
        projection_data_bytes().ok_or("Joined projection partition underflow")?,
        None,
        |_| Ok(BackendStorage::External),
    )
    .err()
    .ok_or("Complete command limit bypassed")?;
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

#[test]
fn configured_japanese_locale_reaches_production_report_view_work_without_web_content()
-> Result<(), Box<dyn Error>> {
    use crate::production::diagnostic_helper::reporting::ReportingWork;
    use sugarsubstitute_contracts::{
        configuration::LocaleId,
        incident::*,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
    };
    let root = bytes(&r"E:\comfyui".encode_utf16().collect::<Vec<_>>());
    let interpreter = bytes(&r"E:\python.exe".encode_utf16().collect::<Vec<_>>());
    let original =
        managed_view(&root, &interpreter, &[], 0, "ja").map_err(|cause| cause as Box<dyn Error>)?;
    let projected = fields::project(
        original,
        &PathBuf::new(),
        projection_data_bytes().ok_or("Joined projection partition underflow")?,
        None,
        |_| Ok(BackendStorage::External),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(projected.locale.as_str(), "ja");
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(1)?,
        ReportSessionId::new(1)?,
    );
    let mut receiving = super::super::incident_reporting::IncidentReporting::new();
    assert!(receiving.view_work(binding).is_err());
    receiving.configure_locale(projected.locale)?;
    let ReportingWork::View { binding: actual, locale, root_capture_degraded } =
        receiving.view_work(binding)?
    else {
        return Err("Report view work changed kind".into());
    };
    assert_eq!(actual, binding);
    assert_eq!(locale, "ja");
    assert!(!root_capture_degraded);
    assert!(receiving.configure_locale(LocaleId::new("en")?).is_err());
    assert!(matches!(receiving.view_work(binding), Ok(ReportingWork::View { locale: "ja", .. })));
    Ok(())
}

/// Windows-shaped snapshot: `variables` entries whose text totals about `units` UTF-16 units.
fn environment_bytes(variables: usize, units: usize) -> Vec<u8> {
    let each = units / variables;
    let mut block: Vec<u16> = Vec::new();
    for index in 0..variables {
        let name = format!("VARIABLE_{index:04}=");
        let value = "v".repeat(each.saturating_sub(name.len() + 1));
        block.extend(name.encode_utf16());
        block.extend(value.encode_utf16());
        block.push(0);
    }
    block.push(0);
    bytes(&block)
}

/// Project a managed launch the way an installed host sends it: long installation paths, one
/// ComfyUI beside them, the output folder setup chose as its argument, the profile folder an
/// earlier launch settled, and the caller's real environment. Returns the unused policy quota.
fn project_installed_managed(environment: &[u8]) -> Result<u64, Box<dyn Error + Send + Sync>> {
    use sugarsubstitute_contracts::{
        configuration::{Presentation, Theme},
        coordinator_entry::{EntryArgumentsView, EntryEnvironmentView, EntryFrontendView},
    };
    let install = r"D:\SugarSubstitute-Rust\canvas-integration\recovery\install-proof\installs\real-03\SugarSubstitute";
    let wide = |tail: &str| bytes(&format!(r"{install}\{tail}").encode_utf16().collect::<Vec<_>>());
    let root = wide(r"releases\generation\comfyui");
    let interpreter = wide(r"releases\generation\python\python.exe");
    let profile = wide(r"user\profile");
    let root = EntryWindowsPathView::from_le_bytes(&root)?;
    let interpreter = EntryWindowsPathView::from_le_bytes(&interpreter)?;
    let profile = EntryWindowsPathView::from_le_bytes(&profile)?;
    let mut arguments = Vec::new();
    for argument in ["--output-directory".to_owned(), format!(r"{install}\..\outputs")] {
        arguments.extend_from_slice(&u32::try_from(argument.len())?.to_le_bytes());
        arguments.extend_from_slice(argument.as_bytes());
    }
    let view = EntryLaunchRequestView::new(
        EntryConnectionView::Managed {
            root,
            interpreter,
            arguments: EntryArgumentsView::from_encoded(&arguments, 2)?,
            port: std::num::NonZeroU16::new(8188).ok_or("Invalid port")?,
        },
        EntryFrontendView::new(root, root, root, root),
        profile,
        "Profile_1",
        Presentation::new("en", Theme::Dark, false)?,
        EntryEnvironmentView::from_le_bytes(environment)?,
    )?;
    let capture = PathBuf::from(format!(r"{install}\user\capture"));
    let available = projection_data_bytes().ok_or("Joined projection partition underflow")?;
    let settled = PathBuf::from(format!(r"{install}\user\desktop-data\profile\Profile_1"));
    let projected = fields::project(view, &capture, available, Some(&settled), |_| {
        Ok(BackendStorage::External)
    })?;
    let (claim, _scope) = super::super::funding::tests::admitted(PROJECTION_BYTES, available)?;
    let mut input = ProjectionInput::without_request();
    input.capture_root = capture;
    input._charge = Some(claim);
    let installation =
        sugarsubstitute_contracts::installation::InstallationId::new("a".repeat(32))?;
    let (_data, policy) = capacity::funding(&mut input, &projected, &installation)?;
    Ok(policy.bytes())
}

/// Every installed launch is managed, and a normal Windows environment holds thousands of units.
#[test]
fn installed_managed_projection_admits_typical_and_widest_environments()
-> Result<(), Box<dyn Error + Send + Sync>> {
    // A typical account environment, a developer-shell environment, and the widest block the
    // native owner still admits beside its storage overrides.
    for (variables, units) in [(130, 9_300), (200, 20_700), (850, 27_500)] {
        let policy = project_installed_managed(&environment_bytes(variables, units))?;
        assert!(policy > 0, "no policy quota left for {units} environment units");
    }
    Ok(())
}

/// The reservation still protects the partition: a path far beyond the lifecycle's share rejects.
#[test]
fn managed_projection_still_rejects_input_beyond_its_partition_as_overloaded()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let long = format!(r"E:\{}", "x".repeat(20_000));
    let root = bytes(&long.encode_utf16().collect::<Vec<_>>());
    let interpreter = bytes(&r"E:\python.exe".encode_utf16().collect::<Vec<_>>());
    let view = managed_view(&root, &interpreter, &[], 0, "en")?;
    let cause = fields::project(
        view,
        &PathBuf::new(),
        projection_data_bytes().ok_or("Joined projection partition underflow")?,
        None,
        |_| Ok(BackendStorage::External),
    )
    .err()
    .ok_or("Oversized managed paths were admitted")?;
    assert!(matches!(
        cause.downcast_ref::<sugarsubstitute_contracts::lifecycle::LifecycleRejection>(),
        Some(sugarsubstitute_contracts::lifecycle::LifecycleRejection::Overloaded)
    ));
    Ok(())
}
