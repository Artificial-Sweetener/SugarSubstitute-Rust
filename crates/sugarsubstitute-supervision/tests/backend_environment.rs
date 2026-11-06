// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize the managed child environment without ambient changes or child processes.
#![cfg(windows)]
use std::error::Error;
use sugarsubstitute_contracts::configuration::LaunchConfiguration;
use sugarsubstitute_supervision::setup_storage::BackendStorage;
use sugarsubstitute_supervision::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
type TestResult = Result<(), Box<dyn Error>>;

fn snapshot(lines: &[&str]) -> Vec<u16> {
    let mut result = Vec::new();
    for line in lines {
        result.extend(line.encode_utf16());
        result.push(0);
    }
    result.push(0);
    if lines.is_empty() {
        result.push(0);
    }
    result
}
fn configuration(manager: bool) -> Result<LaunchConfiguration, Box<dyn Error>> {
    Ok(LaunchConfiguration::new(
        r"\\?\E:\ComfyUI".into(),
        r"\\?\E:\ComfyUI\.venv\Scripts\python.exe".into(),
        if manager { vec!["--enable-manager".into()] } else { Vec::new() },
        8188,
    )?)
}
fn storage() -> &'static std::path::Path {
    std::path::Path::new(r"E:\install\user\desktop-data")
}
fn value(environment: &LaunchEnvironment, name: &str) -> Option<String> {
    environment.as_utf16().split(|unit| *unit == 0).find_map(|line| {
        let text = String::from_utf16_lossy(line);
        let (key, value) = text.split_once('=')?;
        key.eq_ignore_ascii_case(name).then(|| value.to_owned())
    })
}

#[test]
fn selected_python_precedes_original_path_and_desktop_snapshot_stays_unchanged() -> TestResult {
    let input = snapshot(&[
        "z=東京😀",
        "pAtH=E:\\tools;E:\\user scripts",
        "SUGARSUBSTITUTE_CAPABILITY=private",
    ]);
    let original = input.clone();
    let desktop = LaunchEnvironment::from_snapshot_utf16(&input, &[])?;
    let backend = LaunchEnvironment::for_managed_backend(
        &input,
        &configuration(false)?,
        BackendStorage::External,
    )?;
    assert_eq!(
        value(&backend, "PATH").as_deref(),
        Some(r"E:\ComfyUI\.venv\Scripts;E:\tools;E:\user scripts")
    );
    assert_eq!(value(&desktop, "PATH").as_deref(), Some(r"E:\tools;E:\user scripts"));
    assert_eq!(value(&backend, "z").as_deref(), Some("東京😀"));
    assert!(value(&backend, "SUGARSUBSTITUTE_CAPABILITY").is_none());
    assert!(value(&desktop, "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER").is_none());
    assert_eq!(input, original);
    assert!(backend.as_utf16().ends_with(&[0, 0]));
    let raw_path = [80, 65, 84, 72, 61, 0xd800, 0, 0];
    let raw_backend = LaunchEnvironment::for_managed_backend(
        &raw_path,
        &configuration(false)?,
        BackendStorage::External,
    )?;
    assert!(raw_backend.as_utf16().contains(&0xd800));
    assert_eq!(raw_backend.try_copy()?.as_utf16(), raw_backend.as_utf16());
    Ok(())
}

#[test]
fn setup_tool_storage_replaces_inherited_account_and_library_locations() -> TestResult {
    let names = [
        "TEMP",
        "TMP",
        "TMPDIR",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_CACHE_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "UV_CACHE_DIR",
        "UV_PYTHON_INSTALL_DIR",
        "UV_PYTHON_CACHE_DIR",
        "PIP_CACHE_DIR",
        "npm_config_cache",
        "npm_config_logs_dir",
        "PYTHONUSERBASE",
        "PYTHONPYCACHEPREFIX",
        "HF_HOME",
        "HF_HUB_CACHE",
        "HUGGINGFACE_HUB_CACHE",
        "HF_ASSETS_CACHE",
        "HF_XET_CACHE",
        "TRANSFORMERS_CACHE",
        "TORCH_HOME",
        "TORCHINDUCTOR_CACHE_DIR",
        "TRITON_CACHE_DIR",
        "NUMBA_CACHE_DIR",
        "CUDA_CACHE_PATH",
        "MPLCONFIGDIR",
    ];
    let inherited = names.map(|name| format!("{name}=C:\\outside"));
    let mut lines = inherited.iter().map(String::as_str).collect::<Vec<_>>();
    lines.push(r"pip_log=C:\outside\pip.log");
    let input = snapshot(&lines);
    let backend = LaunchEnvironment::for_local_storage(&input, storage())?;
    for name in names {
        assert_eq!(value(&backend, name).as_deref(), storage().to_str(), "{name}");
    }
    assert_eq!(value(&backend, "PIP_LOG").as_deref(), storage().join("pip.log").to_str());
    for name in ["UV_NO_CACHE", "PIP_NO_CACHE_DIR", "PYTHONDONTWRITEBYTECODE", "PYTHONNOUSERSITE"] {
        assert_eq!(value(&backend, name).as_deref(), Some("1"));
    }
    assert!(
        LaunchEnvironment::for_local_storage(&input, std::path::Path::new("relative")).is_err()
    );
    assert!(
        LaunchEnvironment::for_local_storage(&input, &storage().join("x".repeat(32767))).is_err()
    );
    Ok(())
}

#[test]
fn external_comfy_keeps_captured_storage_while_retaining_backend_launch_mechanics() -> TestResult {
    let settings = [
        r"HOME=E:\ordinary-home",
        r"USERPROFILE=C:\Users\selected",
        r"APPDATA=C:\Users\selected\AppData\Roaming",
        r"LOCALAPPDATA=C:\Users\selected\AppData\Local",
        r"HF_HOME=E:\shared-huggingface",
        r"TORCH_HOME=E:\shared-torch",
        r"TEMP=D:\selected-temp",
        r"PIP_LOG=E:\selected-pip.log",
        r"npm_config_cache=E:\selected-node-cache",
        "PYTHONIOENCODING=latin-1",
        r"PATH=E:\existing-tools",
    ];
    let input = snapshot(&settings);
    let original = input.clone();
    let backend = LaunchEnvironment::for_managed_backend(
        &input,
        &configuration(false)?,
        BackendStorage::External,
    )?;
    for setting in &settings[..9] {
        let (name, expected) = setting.split_once('=').ok_or("Setting separator missing")?;
        assert_eq!(value(&backend, name).as_deref(), Some(expected));
    }
    assert_eq!(
        value(&backend, "PATH").as_deref(),
        Some(r"E:\ComfyUI\.venv\Scripts;E:\existing-tools")
    );
    assert_eq!(value(&backend, "PYTHONIOENCODING").as_deref(), Some("utf-8"));
    assert_eq!(value(&backend, "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER").as_deref(), Some("1"));
    assert_eq!(input, original);
    Ok(())
}

#[test]
fn local_storage_removes_browser_storage_overrides_before_environment_creation() -> TestResult {
    let input = snapshot(&[
        r"webview2_user_data_folder=C:\outside",
        r"WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--log-file=C:\outside.log",
        "HTTPS_PROXY=http://proxy.invalid",
    ]);
    let environment = LaunchEnvironment::for_local_storage(&input, storage())?;
    assert!(value(&environment, "WEBVIEW2_USER_DATA_FOLDER").is_none());
    assert!(value(&environment, "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none());
    assert_eq!(value(&environment, "TEMP").as_deref(), storage().to_str());
    assert_eq!(value(&environment, "HTTPS_PROXY").as_deref(), Some("http://proxy.invalid"));
    assert!(
        value(&LaunchEnvironment::from_snapshot_utf16(&input, &[])?, "WEBVIEW2_USER_DATA_FOLDER")
            .is_some()
    );
    Ok(())
}

#[test]
fn captured_output_uses_utf8_and_non_manager_environment_keeps_ambient_manager_values() -> TestResult
{
    let backend = LaunchEnvironment::for_managed_backend(
        &snapshot(&[
            "PYTHONIOENCODING=latin-1",
            "PYTHONUTF8=0",
            "COMFYUI_PATH=existing",
            "GIT_PYTHON_REFRESH=warn",
            "CM_USE_PYGIT2=1",
        ]),
        &configuration(false)?,
        BackendStorage::External,
    )?;
    assert_eq!(value(&backend, "PATH").as_deref(), Some(r"E:\ComfyUI\.venv\Scripts;"));
    assert_eq!(value(&backend, "PYTHONIOENCODING").as_deref(), Some("utf-8"));
    assert_eq!(value(&backend, "PYTHONUTF8").as_deref(), Some("0"));
    assert_eq!(value(&backend, "COMFYUI_PATH").as_deref(), Some("existing"));
    assert_eq!(value(&backend, "GIT_PYTHON_REFRESH").as_deref(), Some("warn"));
    assert_eq!(value(&backend, "CM_USE_PYGIT2").as_deref(), Some("1"));
    Ok(())
}

#[test]
fn explicit_manager_uses_operational_root_utf8_defaults_and_no_unprobed_pygit2() -> TestResult {
    for (lines, expected_utf8) in
        [(vec!["CM_USE_PYGIT2=1"], "1"), (vec!["PYTHONUTF8=", "CM_USE_PYGIT2=1"], "")]
    {
        let backend = LaunchEnvironment::for_managed_backend(
            &snapshot(&lines),
            &configuration(true)?,
            BackendStorage::External,
        )?;
        assert_eq!(value(&backend, "COMFYUI_PATH").as_deref(), Some(r"E:\ComfyUI"));
        assert_eq!(value(&backend, "PYTHONUTF8").as_deref(), Some(expected_utf8));
        assert_eq!(value(&backend, "PYTHONIOENCODING").as_deref(), Some("utf-8"));
        assert_eq!(value(&backend, "GIT_PYTHON_REFRESH").as_deref(), Some("quiet"));
        assert!(value(&backend, "CM_USE_PYGIT2").is_none());
    }
    Ok(())
}

#[test]
fn fixed_tts_marker_does_not_allow_generic_private_insertion_or_caller_values() -> TestResult {
    for name in [
        "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER",
        "sugarsubstitute_other",
        "SUBSTITUTE_SECRET",
        "SUGAR_SUBSTITUTE_TOKEN",
    ] {
        let name: Vec<u16> = name.encode_utf16().collect();
        assert!(matches!(
            LaunchEnvironment::from_snapshot_utf16(
                &[0, 0],
                &[EnvironmentOverride::Set { name: &name, value: &[49] }]
            ),
            Err(LaunchEnvironmentError::PrivateVariable)
        ));
    }
    let backend = LaunchEnvironment::for_managed_backend(
        &snapshot(&["SUGARSUBSTITUTE_SKIP_TTS_INSTALLER=0", "SUGARSUBSTITUTE_OTHER=ignored"]),
        &configuration(false)?,
        BackendStorage::External,
    )?;
    assert_eq!(value(&backend, "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER").as_deref(), Some("1"));
    assert!(value(&backend, "SUGARSUBSTITUTE_OTHER").is_none());
    Ok(())
}

#[test]
fn owned_backend_copy_preserves_its_fixed_semantic_and_original_units() -> TestResult {
    let backend = LaunchEnvironment::for_managed_backend(
        &[0, 0],
        &configuration(true)?,
        BackendStorage::External,
    )?;
    let copied = backend.try_copy()?;
    assert_eq!(copied.as_utf16(), backend.as_utf16());
    assert_eq!(value(&copied, "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER").as_deref(), Some("1"));
    let ordinary = LaunchEnvironment::from_snapshot_utf16(copied.as_utf16(), &[])?;
    assert!(value(&ordinary, "SUGARSUBSTITUTE_SKIP_TTS_INSTALLER").is_none());
    assert!(copied.allocated_bytes()? <= backend.copy_storage_bytes()?);
    Ok(())
}

#[test]
fn malformed_duplicate_and_oversized_managed_environments_reject_before_publication() -> TestResult
{
    let configuration = configuration(false)?;
    assert!(matches!(
        LaunchEnvironment::for_managed_backend(&[0], &configuration, BackendStorage::External),
        Err(LaunchEnvironmentError::InvalidSnapshot)
    ));
    assert!(matches!(
        LaunchEnvironment::for_managed_backend(
            &snapshot(&["PATH=one", "path=two"]),
            &configuration,
            BackendStorage::External
        ),
        Err(LaunchEnvironmentError::DuplicateSnapshot)
    ));
    let long_path = format!("PATH={}", "x".repeat(32750));
    assert!(
        LaunchEnvironment::for_managed_backend(
            &snapshot(&[&long_path]),
            &configuration,
            BackendStorage::External
        )
        .is_err()
    );
    let variables: Vec<_> = (0..1022).map(|index| format!("K{index:04}=v")).collect();
    let lines: Vec<_> = variables.iter().map(String::as_str).collect();
    assert!(
        LaunchEnvironment::for_managed_backend(
            &snapshot(&lines[..1021]),
            &configuration,
            BackendStorage::External
        )
        .is_ok()
    );
    assert!(matches!(
        LaunchEnvironment::for_managed_backend(
            &snapshot(&lines),
            &configuration,
            BackendStorage::External
        ),
        Err(LaunchEnvironmentError::TooManyVariables)
    ));
    Ok(())
}
