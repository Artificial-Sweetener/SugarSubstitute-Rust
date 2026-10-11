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
