// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify effective arguments without filesystem or process effects.
use super::*;
use sugarsubstitute_contracts::configuration::{MAX_ARGUMENT_BYTES, MAX_ARGUMENTS};
pub(super) fn requested(args: Vec<String>) -> Result<LaunchConfiguration, ConfigurationError> {
    #[cfg(windows)]
    let (root, python) = (r"E:\Comfy café 🙂", r"E:\Python\python.exe");
    #[cfg(not(windows))]
    let (root, python) = ("/comfy café 🙂", "/python/bin/python");
    LaunchConfiguration::new(root.into(), python.into(), args, 8188)
}
fn external(requested: &LaunchConfiguration) -> Result<LaunchConfiguration, ManagedLaunchError> {
    effective_managed_launch(requested, BackendDataDisposition::External, requested.root())
}
#[test]
fn constructs_exact_managed_prefix_and_preserves_supported_values() -> Result<(), Box<dyn Error>> {
    let original = requested(vec![
        "--output-directory".into(),
        "画像 café 🙂".into(),
        "--feature-flag".into(),
        "".into(),
    ])?;
    let effective = external(&original)?;
    assert_eq!(
        &effective.arguments()[..9],
        &[
            "-u",
            "-c",
            EXTERNAL_BOOTSTRAP,
            original.root().join("main.py").to_str().ok_or("script")?,
            "--listen",
            "127.0.0.1",
            "--port",
            "8188",
            "--disable-auto-launch"
        ]
    );
    assert_eq!(&effective.arguments()[9..], original.arguments());
    assert_eq!(effective.root(), original.root());
    assert_eq!(effective.interpreter(), original.interpreter());
    assert_eq!(original.arguments().len(), 4);
    Ok(())
}
#[test]
fn rejects_all_protected_options_and_every_abbreviation_and_equals_form()
-> Result<(), Box<dyn Error>> {
    for protected in options::PROTECTED {
        for length in 3..=protected.len() {
            let name = &protected[..length];
            for token in [name.to_owned(), format!("{name}=private")] {
                let request = requested(vec![token])?;
                assert!(matches!(external(&request), Err(ManagedLaunchError::ProtectedOption)));
            }
        }
    }
    Ok(())
}
#[test]
fn alternate_script_python_switches_and_unbound_positionals_are_rejected()
-> Result<(), Box<dyn Error>> {
    for args in [
        vec!["other.py"],
        vec!["-c", "code"],
        vec!["-m", "module"],
        vec!["--", "main.py"],
        vec![""],
        vec!["--cpu", "other.py"],
        vec!["--not-an-option"],
        vec!["--reserve", "1"],
    ] {
        let request = requested(args.into_iter().map(str::to_owned).collect())?;
        assert!(external(&request).is_err());
    }
    Ok(())
}
#[test]
fn option_arity_rejects_missing_extra_and_flag_values() -> Result<(), Box<dyn Error>> {
    for args in [
        vec!["--output-directory"],
        vec!["--cpu=true"],
        vec!["--preview-size", "512", "extra"],
        vec!["--extra-model-paths-config"],
        vec!["--output-directory", "--port", "99"],
        vec!["--output-directory", "-1."],
    ] {
        assert!(external(&requested(args.into_iter().map(str::to_owned).collect())?).is_err());
    }
    Ok(())
}
#[test]
fn option_value_text_is_preserved_without_treating_equals_content_as_an_option()
-> Result<(), Box<dyn Error>> {
    let args = vec![
        "--feature-flag=--port=42",
        "--directml",
        "-1",
        "--async-offload",
        "--fast",
        "fp16_accumulation",
        "cublas_ops",
        "--cache-ram",
        "-1.5",
        "2",
        "--verbose",
        "INFO",
        "画像 log.txt",
    ];
    let request = requested(args.into_iter().map(str::to_owned).collect())?;
    let effective = external(&request)?;
    assert_eq!(&effective.arguments()[MANAGED_PREFIX_ARGUMENTS..], request.arguments());
    Ok(())
}
#[test]
fn complete_command_argument_count_includes_managed_interpreter_and_server_entries()
-> Result<(), Box<dyn Error>> {
    external(&requested(vec!["--cpu".into(); MAX_ARGUMENTS - MANAGED_PREFIX_ARGUMENTS])?)?;
    let request = requested(vec!["--cpu".into(); MAX_ARGUMENTS - MANAGED_PREFIX_ARGUMENTS + 1])?;
    assert!(matches!(
        external(&request),
        Err(ManagedLaunchError::Configuration(ConfigurationError::TooManyArguments))
    ));
    Ok(())
}
#[test]
fn complete_command_total_bytes_includes_prefix_and_script() -> Result<(), Box<dyn Error>> {
    let mut args = Vec::new();
    for _ in 0..16 {
        args.push("--feature-flag".into());
        args.push("x".repeat(MAX_ARGUMENT_BYTES - 14));
    }
    let request = requested(args)?;
    assert!(matches!(
        external(&request),
        Err(ManagedLaunchError::Configuration(ConfigurationError::ArgumentsTooLarge))
    ));
    Ok(())
}
#[test]
fn rejected_option_diagnostics_do_not_render_private_argument_values() -> Result<(), Box<dyn Error>>
{
    let error = external(&requested(vec!["--port=private-secret".into()])?).err().ok_or("error")?;
    assert!(!format!("{error} {error:?}").contains("private-secret"));
    Ok(())
}

#[test]
fn owned_model_validation_keeps_server_argv_and_extra_values_while_external_is_unchanged()
-> Result<(), Box<dyn Error>> {
    let original = requested(vec!["--base-directory=画像 café".into(), "--cpu".into()])?;
    let external = external(&original)?;
    let cubes = original.root().join("custom_nodes/SugarCubes");
    let owned =
        effective_managed_launch(&original, BackendDataDisposition::InstallationOwned, &cubes)?;
    assert_eq!(owned.arguments()[2], OWNED_BOOTSTRAP);
    assert_eq!(owned.arguments()[3], cubes.to_str().ok_or("cube path")?);
    assert_eq!(server_arguments(&owned), server_arguments(&external));
    assert_eq!(&owned.arguments()[MAX_MANAGED_PREFIX_ARGUMENTS..], original.arguments());
    assert_eq!(external.arguments()[2], EXTERNAL_BOOTSTRAP);
    assert!(!EXTERNAL_BOOTSTRAP.contains("read_preserved_selection"));
    assert!(OWNED_BOOTSTRAP.contains("read_preserved_selection"));
    assert!(OWNED_BOOTSTRAP.len() <= MAX_ARGUMENT_BYTES);
    assert_eq!(original.arguments(), ["--base-directory=画像 café", "--cpu"]);
    Ok(())
}

#[test]
fn owned_prefix_uses_existing_argument_limits_and_requires_explicit_absolute_cube_path()
-> Result<(), Box<dyn Error>> {
    let original = requested(vec!["--cpu".into(); MAX_ARGUMENTS - MAX_MANAGED_PREFIX_ARGUMENTS])?;
    effective_managed_launch(
        &original,
        BackendDataDisposition::InstallationOwned,
        original.root(),
    )?;
    let overflow =
        requested(vec!["--cpu".into(); MAX_ARGUMENTS - MAX_MANAGED_PREFIX_ARGUMENTS + 1])?;
    assert!(matches!(
        effective_managed_launch(
            &overflow,
            BackendDataDisposition::InstallationOwned,
            overflow.root()
        ),
        Err(ManagedLaunchError::Configuration(ConfigurationError::TooManyArguments))
    ));
    assert!(
        effective_managed_launch(
            &original,
            BackendDataDisposition::InstallationOwned,
            Path::new("relative-cubes")
        )
        .is_err()
    );
    // The unused external value grants no API check, path interpretation or altered argv.
    assert_eq!(
        effective_managed_launch(
            &original,
            BackendDataDisposition::External,
            Path::new("relative-cubes")
        )?,
        external(&original)?
    );
    Ok(())
}
