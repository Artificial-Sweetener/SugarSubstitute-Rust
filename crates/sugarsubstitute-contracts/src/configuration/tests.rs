// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Protect semantic configuration admission and exact requested argument spelling.

use super::*;

/// Supply absolute lexical paths without reading or creating their targets.
fn paths() -> (PathBuf, PathBuf) {
    #[cfg(windows)]
    {
        (PathBuf::from(r"E:\ComfyUI"), PathBuf::from(r"E:\Python\python.exe"))
    }
    #[cfg(not(windows))]
    {
        (PathBuf::from("/comfy"), PathBuf::from("/python/bin/python"))
    }
}

/// Construct one isolated launch value with caller-supplied argument bytes.
fn launch(arguments: Vec<String>) -> Result<LaunchConfiguration, ConfigurationError> {
    let (root, interpreter) = paths();
    LaunchConfiguration::new(root, interpreter, arguments, 8188)
}

#[test]
fn launch_retains_unicode_arguments_and_redacts_diagnostics()
-> Result<(), Box<dyn std::error::Error>> {
    let arguments = vec![
        "".into(),
        "画像 café 🙂".into(),
        "--token=private-credential".into(),
        "$literal; & value".into(),
    ];
    let value = launch(arguments.clone())?;
    assert_eq!(value.arguments(), arguments);
    assert_eq!(value.port().get(), 8188);
    let diagnostic = format!("{value:?}");
    assert!(!diagnostic.contains("private-credential"));
    assert!(!diagnostic.contains("ComfyUI"));
    Ok(())
}

#[test]
fn launch_argument_limits_include_exact_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    launch(vec!["".into(); MAX_ARGUMENTS])?;
    assert_eq!(
        launch(vec!["".into(); MAX_ARGUMENTS + 1]),
        Err(ConfigurationError::TooManyArguments)
    );
    launch(vec!["é".repeat(MAX_ARGUMENT_BYTES / 2)])?;
    assert_eq!(
        launch(vec!["é".repeat(MAX_ARGUMENT_BYTES / 2 + 1)]),
        Err(ConfigurationError::ArgumentTooLong)
    );
    launch(vec!["a".repeat(MAX_ARGUMENT_BYTES); 16])?;
    assert_eq!(
        launch(vec!["a".repeat(MAX_ARGUMENT_BYTES); 17]),
        Err(ConfigurationError::ArgumentsTooLarge)
    );
    assert_eq!(launch(vec!["abc\0def".into()]), Err(ConfigurationError::ArgumentContainsNul));
    Ok(())
}

#[test]
fn launch_requires_absolute_paths_and_nonzero_port() {
    let (root, interpreter) = paths();
    assert_eq!(
        LaunchConfiguration::new("relative".into(), interpreter.clone(), vec![], 8188),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    assert_eq!(
        LaunchConfiguration::new(root.clone(), "python".into(), vec![], 8188),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    assert_eq!(
        LaunchConfiguration::new(
            root.join("..").join("outside"),
            interpreter.clone(),
            vec![],
            8188
        ),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    assert_eq!(
        LaunchConfiguration::new(root.clone(), root.join("bad\0path"), vec![], 8188),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    assert_eq!(
        LaunchConfiguration::new(root, interpreter, vec![], 0),
        Err(ConfigurationError::InvalidPort)
    );
}

#[test]
fn profile_names_cannot_be_paths_or_ambiguous_identities() -> Result<(), Box<dyn std::error::Error>>
{
    for value in ["a", "Desktop_2-yes", &"a".repeat(64)] {
        assert_eq!(ProfileId::new(value.into())?.as_str(), value);
    }
    for value in ["", ".", "..", "a/b", "a\\b", "a:b", "a b", "日本語", &"a".repeat(65)] {
        assert_eq!(ProfileId::new(value.into()), Err(ConfigurationError::InvalidProfile));
    }
    Ok(())
}

#[test]
fn presentation_uses_only_the_authoritative_locale_registry()
-> Result<(), Box<dyn std::error::Error>> {
    for locale in crate::localization::LOCALES {
        for theme in [Theme::Dark, Theme::Light] {
            let presentation = Presentation::new(locale.id, theme, true)?;
            assert_eq!(presentation.locale().as_str(), locale.id);
            assert_eq!(presentation.theme(), theme);
            assert!(presentation.reduced_motion());
        }
    }
    for value in ["EN", "en-US", "zh", "", "unknown"] {
        assert_eq!(LocaleId::new(value), Err(ConfigurationError::UnsupportedLocale));
    }
    Ok(())
}

#[test]
fn attachment_admits_exact_loopback_roots_and_matching_ports()
-> Result<(), Box<dyn std::error::Error>> {
    for (url, host) in [
        ("http://127.0.0.1:8188/", LoopbackHost::Ipv4),
        ("http://[::1]:8188", LoopbackHost::Ipv6),
        ("http://localhost:8188/", LoopbackHost::Localhost),
    ] {
        let origin = LoopbackOrigin::parse(url, 8188)?;
        assert_eq!(origin.host(), host);
        assert_eq!(origin.port().get(), 8188);
    }
    LoopbackOrigin::parse("http://localhost/", 80)?;
    assert_eq!(
        LoopbackOrigin::parse("http://localhost/", 8188),
        Err(ConfigurationError::AttachPortMismatch)
    );
    assert_eq!(
        LoopbackOrigin::parse("http://localhost:8189/", 8188),
        Err(ConfigurationError::AttachPortMismatch)
    );
    assert_eq!(
        LoopbackOrigin::parse("http://localhost:0/", 8188),
        Err(ConfigurationError::InvalidPort)
    );
    Ok(())
}

#[test]
fn attachment_rejects_remote_hosts_credentials_and_url_payloads() {
    for url in [
        "https://localhost:8188/",
        "HTTP://localhost:8188/",
        "http://LOCALHOST:8188/",
        "http://localhost.:8188/",
        "http://127.0.0.2:8188/",
        "http://127.1:8188/",
        "http://[0:0:0:0:0:0:0:1]:8188/",
        "http://example.com:8188/",
        "http://user:secret@localhost:8188/",
        "http://localhost:8188/path",
        "http://localhost:8188//",
        "http://localhost:8188/?token=secret",
        "http://localhost:8188/#secret",
        "http://localhost:08188/",
        "http://localhost:65536/",
        "http://localhost:8188/ ",
        "http://localhost:8188%2f",
    ] {
        let error = LoopbackOrigin::parse(url, 8188).err();
        assert!(error.is_some(), "accepted {url}");
        assert!(error.is_none_or(|error| !error.to_string().contains("secret")));
    }
}

#[test]
fn borrowed_profile_and_arguments_preserve_owned_grammar_error_order() {
    for value in ["", "a/b", "valid_1"] {
        assert_eq!(ProfileId::validate(value), ProfileId::new(value.to_owned()).map(|_| ()));
    }
    let excessive = "x".repeat(MAX_ARGUMENT_BYTES);
    let mut arguments = vec![excessive.as_str(); 17];
    arguments.push("bad\0argument");
    assert_eq!(
        LaunchConfiguration::validate_arguments(arguments.iter().copied()),
        Err(ConfigurationError::ArgumentContainsNul)
    );
    assert_eq!(
        LaunchConfiguration::validate_arguments(["bad\0argument"; 65]),
        Err(ConfigurationError::TooManyArguments)
    );
}

#[test]
fn managed_capacity_includes_spare_paths_vector_and_argument_storage()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut root, mut interpreter) = paths();
    root.reserve(1024);
    interpreter.reserve(2048);
    let mut argument = String::with_capacity(4096);
    argument.push_str("exact text");
    let mut arguments = Vec::with_capacity(8);
    arguments.push(argument);
    let expected = root.capacity()
        + interpreter.capacity()
        + arguments.capacity() * size_of::<String>()
        + arguments[0].capacity();
    let value = LaunchConfiguration::new(root, interpreter, arguments, 8188)?;
    assert_eq!(value.allocated_bytes(), Some(expected));
    assert_eq!(value.arguments(), ["exact text"]);
    Ok(())
}

#[cfg(windows)]
#[test]
fn managed_paths_preserve_unpaired_windows_units_without_native_inspection()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let units = [69, 58, 92, 0xd800, 97, 0xdc01];
    let root = PathBuf::from(std::ffi::OsString::from_wide(&units));
    let (_, interpreter) = paths();
    assert!(root.to_str().is_none());
    let value = LaunchConfiguration::new(root, interpreter, vec![], 8188)?;
    assert_eq!(value.root().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    Ok(())
}

#[cfg(windows)]
#[test]
fn managed_windows_path_units_enforce_exact_limit_nul_and_traversal() {
    use std::os::windows::ffi::OsStringExt;
    let mut units = vec![69, 58, 92];
    units.resize(32766, 97);
    assert!(
        LaunchConfiguration::validate_path(Path::new(&std::ffi::OsString::from_wide(&units)))
            .is_ok()
    );
    units.push(97);
    assert_eq!(
        LaunchConfiguration::validate_path(Path::new(&std::ffi::OsString::from_wide(&units))),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    units.truncate(4);
    units[3] = 0;
    assert_eq!(
        LaunchConfiguration::validate_path(Path::new(&std::ffi::OsString::from_wide(&units))),
        Err(ConfigurationError::InvalidLaunchPath)
    );
    assert_eq!(
        LaunchConfiguration::validate_path(Path::new(r"E:\root\..\outside")),
        Err(ConfigurationError::InvalidLaunchPath)
    );
}
