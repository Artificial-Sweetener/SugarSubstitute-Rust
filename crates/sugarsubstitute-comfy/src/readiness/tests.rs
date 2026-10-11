// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise complete response decoding and compare claimed identity without native effects.
use super::*;
use serde_json::{Value, json};
use sugarsubstitute_contracts::configuration::MAX_ARGUMENTS;
fn system(argv: &[&str]) -> Value {
    json!({"system":{"os":"win32","ram_total":68719476736u64,"ram_free":40000000000u64,
        "comfyui_version":"0.29.0","required_frontend_version":"1.47.11",
        "installed_templates_version":"1.0.0","required_templates_version":"1.0.0",
        "comfy_package_versions":[{"name":"comfyui-frontend-package","installed":"1.47.11","required":"1.47.11"}],
        "python_version":"3.12.0","pytorch_version":"2.9.0","embedded_python":false,
        "deploy_environment":"local","argv":argv},"devices":[{"name":"GPU","type":"cuda","index":0,"vram_total":34359738368u64}]})
}
fn decode(value: &Value) -> Result<SystemStats, ReadinessError> {
    decode_system_stats(&serde_json::to_vec(value).map_err(ReadinessError::Json)?)
}
fn installed(paths: Vec<String>) -> Result<InstalledFrontendIdentity, ReadinessError> {
    InstalledFrontendIdentity::new("1.47.11".into(), "0.15.0".into(), paths)
}
#[test]
fn actual_system_layout_and_extension_urls_decode_without_hardware_overfit()
-> Result<(), Box<dyn Error>> {
    let stats = decode(&system(&["main.py", "--cpu"]))?;
    let extensions = decode_extensions(
        br#"["/extensions/core/foo.js","/extensions/SugarCubes/comfyui/ui/main.js"]"#,
    )?;
    installed(vec![extensions.paths()[1].clone()])?.compare(&stats, &extensions)?;
    assert_eq!(stats.argv(), ["main.py", "--cpu"]);
    assert_eq!(stats.comfyui_version(), "0.29.0");
    assert_eq!(stats.required_frontend_version(), "1.47.11");
    Ok(())
}
#[test]
fn attached_identity_comparison_does_not_require_managed_argv() -> Result<(), Box<dyn Error>> {
    let stats = decode(&system(&["unrelated-spelling.py", "--port", "9191"]))?;
    let extensions = decode_extensions(br#"["/extensions/SugarCubes/main.js"]"#)?;
    installed(extensions.paths().to_vec())?.compare(&stats, &extensions)?;
    Ok(())
}
#[test]
fn managed_argv_comparison_is_separate_and_excludes_only_python_unbuffered_switch()
-> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let (root, python) = (r"E:\Comfy café", r"E:\Python\python.exe");
    #[cfg(not(windows))]
    let (root, python) = ("/comfy café", "/python/bin/python");
    let request = LaunchConfiguration::new(root.into(), python.into(), vec!["--cpu".into()], 8188)?;
    let effective = crate::launch::effective_managed_launch(&request)?;
    let argv: Vec<&str> = effective.arguments()[1..].iter().map(String::as_str).collect();
    let stats = decode(&system(&argv))?;
    compare_managed_arguments(&stats, &effective)?;
    let mut wrong = system(&argv);
    wrong["system"]["argv"][2] = json!("0.0.0.0");
    assert!(matches!(
        compare_managed_arguments(&decode(&wrong)?, &effective),
        Err(ReadinessError::ManagedArgumentsMismatch)
    ));
    assert!(compare_managed_arguments(&stats, &request).is_err());
    Ok(())
}
#[test]
fn endpoint_body_bounds_are_independent_and_exact() -> Result<(), Box<dyn Error>> {
    let mut stats = serde_json::to_vec(&system(&["main.py"]))?;
    stats.resize(MAX_SYSTEM_STATS_BYTES, b' ');
    decode_system_stats(&stats)?;
    stats.push(b' ');
    assert!(matches!(decode_system_stats(&stats), Err(ReadinessError::TooLarge)));
    let mut extensions = b"[]".to_vec();
    extensions.resize(MAX_EXTENSIONS_BYTES, b' ');
    decode_extensions(&extensions)?;
    extensions.push(b' ');
    assert!(matches!(decode_extensions(&extensions), Err(ReadinessError::TooLarge)));
    Ok(())
}
#[test]
fn seven_hundred_fifty_seven_required_cube_paths_fit_a_larger_full_inventory()
-> Result<(), Box<dyn Error>> {
    let cube: Vec<String> = (0..757)
        .map(|i| format!("/extensions/SugarCubes/comfyui/ui/affordance/FixturePresenter{i}.js"))
        .collect();
    let mut all = cube.clone();
    all.extend(
        (0..600)
            .map(|i| format!("/extensions/other-extension/components/FixtureFrontendModule{i}.js")),
    );
    let bytes = serde_json::to_vec(&all)?;
    assert!(bytes.len() > MAX_SYSTEM_STATS_BYTES && bytes.len() < MAX_EXTENSIONS_BYTES);
    let served = decode_extensions(&bytes)?;
    installed(cube)?.compare(&decode(&system(&["main.py"]))?, &served)?;
    Ok(())
}
#[test]
fn malformed_duplicate_versioned_nonfinite_and_trailing_json_is_rejected()
-> Result<(), Box<dyn Error>> {
    for bytes in [b"{}".as_slice(), b"[] trailing", b"[NaN]", b"[1e999]", b"[", b"\xff"] {
        assert!(decode_system_stats(bytes).is_err());
        assert!(decode_extensions(bytes).is_err());
    }
    let text = serde_json::to_string(&system(&["main.py"]))?;
    for malformed in [
        text.replace("\"os\":\"win32\"", "\"os\":\"win32\",\"os\":\"other\""),
        text.replace("\"system\":{", "\"schema_version\":2,\"system\":{"),
        text.replace("\"devices\":[", "\"unknown\":{\"field\":1,\"field\":2},\"devices\":["),
        format!("{text}{{}}"),
    ] {
        assert!(decode_system_stats(malformed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn extension_duplicate_traversal_query_and_ambiguous_paths_fail() -> Result<(), Box<dyn Error>> {
    for path in [
        "/extensions/x/../main.js",
        "/extensions/x/./main.js",
        "/extensions//main.js",
        "/extensions/x/main.js?token=x",
        "/extensions/x/main.js#x",
        "/extensions/x\\main.js",
        "https://example/main.js",
        "/extensions/x/%2emain.js",
        "/extensions/x/%2Fmain.js",
        "/extensions/x/main.ts",
    ] {
        assert!(decode_extensions(&serde_json::to_vec(&vec![path])?).is_err());
    }
    assert!(decode_extensions(br#"["/extensions/x/a.js","/extensions/x/a.js"]"#).is_err());
    assert!(installed(vec!["/extensions/other/a.js".into()]).is_err());
    Ok(())
}
#[test]
fn expected_frontend_and_required_cube_membership_must_both_match() -> Result<(), Box<dyn Error>> {
    let expected = installed(vec!["/extensions/SugarCubes/main.js".into()])?;
    let served = decode_extensions(br#"["/extensions/SugarCubes/main.js"]"#)?;
    let mut wrong = system(&["main.py"]);
    wrong["system"]["comfy_package_versions"][0]["installed"] = json!("other");
    assert!(matches!(
        expected.compare(&decode(&wrong)?, &served),
        Err(ReadinessError::FrontendMismatch)
    ));
    assert!(matches!(
        expected.compare(&decode(&system(&["main.py"]))?, &decode_extensions(b"[]")?),
        Err(ReadinessError::MissingExtension)
    ));
    assert_eq!(expected.cube_version(), "0.15.0");
    Ok(())
}
#[test]
fn complete_schema_counts_versions_nuls_and_array_nesting_are_bounded() -> Result<(), Box<dyn Error>>
{
    let mut value = system(&["main.py"]);
    value["system"]["argv"] = json!(vec![""; MAX_ARGUMENTS + 1]);
    assert!(decode(&value).is_err());
    value = system(&["main.py\0"]);
    assert!(decode(&value).is_err());
    value = system(&["main.py"]);
    value["system"]["comfyui_version"] = json!("");
    assert!(decode(&value).is_err());
    value = system(&["main.py"]);
    value["system"]["comfy_package_versions"] = json!([
        value["system"]["comfy_package_versions"][0].clone(),
        value["system"]["comfy_package_versions"][0].clone()
    ]);
    assert!(decode(&value).is_err());
    let paths: Vec<String> =
        (0..MAX_EXTENSION_PATHS).map(|i| format!("/extensions/x/{i}.js")).collect();
    decode_extensions(&serde_json::to_vec(&paths)?)?;
    let mut too_many = paths;
    too_many.push("/extensions/x/extra.js".into());
    assert!(decode_extensions(&serde_json::to_vec(&too_many)?).is_err());
    let nested = format!("{}0{}", "[".repeat(34), "]".repeat(34));
    assert!(decode_extensions(nested.as_bytes()).is_err());
    Ok(())
}
#[test]
fn diagnostics_redact_supplied_server_payload_and_retained_cause_is_explicit()
-> Result<(), Box<dyn Error>> {
    let error = decode_system_stats(br#"{"system":"private-credential"}"#).err().ok_or("error")?;
    assert!(!format!("{error} {error:?}").contains("private-credential"));
    assert!(error.source().is_some());
    Ok(())
}

#[test]
fn scalar_types_required_nullable_fields_and_empty_argv_reject() -> Result<(), Box<dyn Error>> {
    let baseline = system(&["main.py"]);
    let mut multiline = baseline.clone();
    multiline["system"]["python_version"] = json!("3.12.0 (main, build)\n[GCC compiler]");
    decode(&multiline)?;
    multiline["system"]["python_version"] = json!("3.12.0\u{0000}");
    assert!(decode(&multiline).is_err());
    for value in [Value::Null, json!(123), json!([])] {
        let mut wrong = baseline.clone();
        wrong["system"]["comfyui_version"] = value;
        assert!(decode(&wrong).is_err());
    }
    let mut wrong = baseline.clone();
    wrong["system"]["argv"] = json!([]);
    assert!(decode(&wrong).is_err());
    wrong = baseline.clone();
    wrong["system"]["comfy_package_versions"][0]
        .as_object_mut()
        .ok_or("package")?
        .remove("installed");
    assert!(decode(&wrong).is_err());
    wrong = baseline;
    wrong["system"]["comfy_package_versions"][0]["installed"] = Value::Null;
    let stats = decode(&wrong)?;
    assert!(matches!(
        installed(vec!["/extensions/SugarCubes/a.js".into()])?
            .compare(&stats, &decode_extensions(br#"["/extensions/SugarCubes/a.js"]"#)?),
        Err(ReadinessError::FrontendMismatch)
    ));
    Ok(())
}

#[test]
fn exact_path_byte_limit_and_unicode_spelling_are_preserved() -> Result<(), Box<dyn Error>> {
    let prefix = "/extensions/SugarCubes/";
    let path = format!("{prefix}{}.js", "a".repeat(MAX_EXTENSION_PATH_BYTES - prefix.len() - 3));
    let inventory = decode_extensions(&serde_json::to_vec(&vec![&path])?)?;
    assert_eq!(inventory.paths(), std::slice::from_ref(&path));
    assert!(decode_extensions(&serde_json::to_vec(&vec![format!("{path}x.js")])?).is_err());
    let unicode = "/extensions/SugarCubes/画像 café 🙂.js";
    let inventory = decode_extensions(&serde_json::to_vec(&vec![unicode])?)?;
    assert_eq!(inventory.paths(), &[unicode]);
    installed(inventory.paths().to_vec())?.compare(&decode(&system(&["main.py"]))?, &inventory)?;
    Ok(())
}
