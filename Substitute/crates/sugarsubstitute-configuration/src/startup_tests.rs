// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify the persisted startup schema before any resource is acquired.

use super::*;

/// Keep a complete explicit schema-one attachment fixture.
const ATTACH: &str = "schema_version = 1\nmode = 'attach'\nport = 8188\nprofile = 'desktop-1'\nlocale = 'en'\ntheme = 'dark'\nreduced_motion = false\n[attach]\norigin = 'http://127.0.0.1:8188/'\n";

/// Build a platform-native absolute launch fixture without creating filesystem state.
fn launch_document(arguments: &str) -> String {
    #[cfg(windows)]
    let paths = "root = 'E:\\ComfyUI'\ninterpreter = 'E:\\Python\\python.exe'\n";
    #[cfg(not(windows))]
    let paths = "root = '/comfy'\ninterpreter = '/python/bin/python'\n";
    format!(
        "schema_version = 1\nmode = 'launch'\nport = 8188\nprofile = 'desktop-2'\nlocale = 'ja'\ntheme = 'light'\nreduced_motion = true\n[launch]\n{paths}arguments = {arguments}\n"
    )
}

#[test]
fn parses_explicit_attach_and_launch_without_losing_unicode() -> Result<(), Box<dyn Error>> {
    let attach = parse_startup_configuration(ATTACH.as_bytes())?;
    assert!(matches!(attach.connection(), ConnectionConfiguration::Attach(_)));
    assert_eq!(attach.profile().as_str(), "desktop-1");
    assert_eq!(attach.presentation().theme(), Theme::Dark);
    assert!(!attach.presentation().reduced_motion());
    let launch = parse_startup_configuration(
        launch_document("['', '画像 café 🙂', '$literal; & value']").as_bytes(),
    )?;
    let ConnectionConfiguration::Launch(request) = launch.connection() else {
        return Err("expected launch".into());
    };
    assert_eq!(request.arguments(), ["", "画像 café 🙂", "$literal; & value"]);
    assert_eq!(request.port().get(), 8188);
    assert_eq!(launch.presentation().locale().as_str(), "ja");
    assert_eq!(launch.presentation().theme(), Theme::Light);
    assert!(launch.presentation().reduced_motion());
    Ok(())
}

#[test]
fn bounded_document_rejects_size_and_invalid_utf8_before_parsing() {
    assert!(matches!(
        parse_startup_configuration(&[0xff]),
        Err(StartupConfigurationError::Utf8(_))
    ));
    assert!(matches!(
        parse_startup_configuration(&vec![b' '; MAX_STARTUP_CONFIGURATION_BYTES + 1]),
        Err(StartupConfigurationError::TooLarge)
    ));
    let mut boundary = ATTACH.as_bytes().to_vec();
    boundary.resize(MAX_STARTUP_CONFIGURATION_BYTES, b' ');
    assert!(parse_startup_configuration(&boundary).is_ok());
}

#[test]
fn malformed_duplicate_unknown_or_missing_schema_fields_are_rejected() {
    for text in [
        "not = [",
        "",
        &ATTACH.replace("port = 8188", "port = 8188\nport = 8188"),
        &ATTACH.replace("port = 8188", "port = 8188\nunknown = 'private-credential'"),
        &format!("{ATTACH}unexpected = true\n"),
        &ATTACH.replace("locale = 'en'\n", ""),
        &ATTACH.replace("theme = 'dark'", "theme = 'Dark'"),
        &ATTACH.replace("reduced_motion = false", "reduced_motion = 'false'"),
    ] {
        assert!(
            matches!(
                parse_startup_configuration(text.as_bytes()),
                Err(StartupConfigurationError::Syntax(_))
            ),
            "accepted {text}"
        );
    }
    assert!(matches!(
        parse_startup_configuration(
            ATTACH.replace("schema_version = 1", "schema_version = 3").as_bytes()
        ),
        Err(StartupConfigurationError::UnsupportedSchema)
    ));
}

#[test]
fn mode_tables_are_exclusive_and_match_the_explicit_mode() {
    let extra =
        format!("{ATTACH}[launch]\nroot = '/comfy'\ninterpreter = '/python'\narguments = []\n");
    for text in [
        extra,
        ATTACH.replace("mode = 'attach'", "mode = 'launch'"),
        ATTACH.split("[attach]").next().unwrap_or_default().into(),
    ] {
        assert!(matches!(
            parse_startup_configuration(text.as_bytes()),
            Err(StartupConfigurationError::ModeTables)
        ));
    }
    let mixed = format!("{ATTACH}arguments = []\n");
    assert!(matches!(
        parse_startup_configuration(mixed.as_bytes()),
        Err(StartupConfigurationError::Syntax(_))
    ));
}

#[test]
fn parsed_fields_pass_domain_validation_and_preserve_causes() {
    for (needle, replacement, cause) in [
        ("port = 8188", "port = 0", ConfigurationError::InvalidPort),
        ("profile = 'desktop-1'", "profile = '../outside'", ConfigurationError::InvalidProfile),
        ("locale = 'en'", "locale = 'EN'", ConfigurationError::UnsupportedLocale),
        (
            "origin = 'http://127.0.0.1:8188/'",
            "origin = 'http://localhost:8189/'",
            ConfigurationError::AttachPortMismatch,
        ),
        (
            "origin = 'http://127.0.0.1:8188/'",
            "origin = 'http://user:private-credential@localhost:8188/'",
            ConfigurationError::InvalidAttachOrigin,
        ),
    ] {
        let error = parse_startup_configuration(ATTACH.replace(needle, replacement).as_bytes());
        assert!(
            matches!(&error, Err(StartupConfigurationError::Invalid(actual)) if *actual == cause)
        );
        if let Err(error) = error {
            assert!(error.source().is_some());
            assert!(!error.to_string().contains("private-credential"));
            assert!(!format!("{error:?}").contains("private-credential"));
        }
    }
    let error = parse_startup_configuration(
        ATTACH.replace("theme = 'dark'", "theme = 'private-credential'").as_bytes(),
    );
    assert!(matches!(error, Err(StartupConfigurationError::Syntax(_))));
    if let Err(error) = error {
        assert!(error.source().and_then(|cause| cause.downcast_ref::<toml::de::Error>()).is_some());
        assert!(!error.to_string().contains("private-credential"));
        assert!(!format!("{error:?}").contains("private-credential"));
    }
}

#[test]
fn parser_applies_launch_argument_and_path_validation() {
    let too_many = format!("[{}]", vec!["''"; 65].join(","));
    let too_long = format!("['{}']", "é".repeat(2049));
    for (arguments, cause) in [
        (too_many, ConfigurationError::TooManyArguments),
        (too_long, ConfigurationError::ArgumentTooLong),
        (r#"["nul\u0000value"]"#.into(), ConfigurationError::ArgumentContainsNul),
    ] {
        assert!(
            matches!(parse_startup_configuration(launch_document(&arguments).as_bytes()), Err(StartupConfigurationError::Invalid(actual)) if actual == cause)
        );
    }
    let document = launch_document("[]")
        .replace("root = 'E:\\ComfyUI'", "root = 'relative'")
        .replace("root = '/comfy'", "root = 'relative'");
    assert!(matches!(
        parse_startup_configuration(document.as_bytes()),
        Err(StartupConfigurationError::Invalid(ConfigurationError::InvalidLaunchPath))
    ));
}

/// Schema-two attachment with a top-level renderer preference line inserted before its tables.
fn schema_two(preference: &str) -> String {
    ATTACH.replacen("schema_version = 1\n", &format!("schema_version = 2\n{preference}"), 1)
}

#[test]
fn schema_two_reads_the_renderer_preference_and_schema_one_reads_as_absent()
-> Result<(), Box<dyn Error>> {
    let stored = |text: &str| CanvasRendererPreference::stored(text);
    assert_eq!(
        parse_startup_configuration(ATTACH.as_bytes())?.canvas_renderer(),
        &CanvasRendererPreference::Absent
    );
    assert_eq!(
        parse_startup_configuration(schema_two("").as_bytes())?.canvas_renderer(),
        &CanvasRendererPreference::Absent
    );
    for text in ["vulkan:10de:2b85", "cpu", "not-a-selector"] {
        let document = schema_two(&format!("canvas_renderer = '{text}'\n"));
        assert_eq!(
            parse_startup_configuration(document.as_bytes())?.canvas_renderer(),
            &stored(text)
        );
    }
    let schema_one =
        ATTACH.replacen("schema_version = 1\n", "schema_version = 1\ncanvas_renderer = 'cpu'\n", 1);
    assert!(matches!(
        parse_startup_configuration(schema_one.as_bytes()),
        Err(StartupConfigurationError::UnsupportedSchema)
    ));
    Ok(())
}

#[test]
fn corrupt_renderer_preferences_are_observable_misses_that_keep_startup_and_bytes()
-> Result<(), Box<dyn Error>> {
    for line in [
        "canvas_renderer = 5\n".to_owned(),
        "canvas_renderer = ['vulkan:10de:2b85']\n".to_owned(),
        format!("canvas_renderer = '{}'\n", "x".repeat(65)),
    ] {
        let bytes = schema_two(&line).into_bytes();
        let original = bytes.clone();
        let configuration = parse_startup_configuration(&bytes)?;
        assert_eq!(configuration.canvas_renderer(), &CanvasRendererPreference::Malformed);
        assert_eq!(configuration.profile().as_str(), "desktop-1");
        assert_eq!(bytes, original);
    }
    Ok(())
}
