// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ambiguous installed claims, hostile paths and incomplete inventories without filesystem effects.
use super::*;
mod ownership;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn metadata_requires_exact_distribution_and_single_version() -> TestResult {
    assert_eq!(
        manifest::distribution_version(b"Name: comfyui_frontend_package\nVersion: 1.47.11\n\n")?,
        "1.47.11"
    );
    for bytes in [
        b"Name: other\nVersion: 1\n".as_slice(),
        b"Name: comfyui_frontend_package\nVersion: 1\nVersion: 2\n",
        b"Name: comfyui_frontend_package\nName: comfyui_frontend_package\nVersion: 1\n",
        b"Name: comfyui_frontend_package\nVersion: \n",
    ] {
        assert!(manifest::distribution_version(bytes).is_err());
    }
    Ok(())
}
#[test]
fn cube_metadata_uses_complete_toml_duplicate_rejection() -> TestResult {
    assert_eq!(
        manifest::cube_version(b"[project]\nname='SugarCubes'\nversion='0.15.0'\n")?,
        "0.15.0"
    );
    for text in [
        "[project]\nname='SugarCubes'\nversion='1'\nversion='2'\n",
        "[project]\nname='other'\nversion='1'\n",
        "[project]\nname='SugarCubes'\nversion=1\n",
    ] {
        assert!(manifest::cube_version(text.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn cube_web_directory_is_literal_and_has_one_supported_declaration() -> TestResult {
    manifest::cube_web_directory(b"WEB_DIRECTORY = \"web\"\n")?;
    for text in [
        "WEB_DIRECTORY = other()",
        "WEB_DIRECTORY = '../web'",
        "WEB_DIRECTORY = 'web'\nWEB_DIRECTORY = 'web'",
        "xWEB_DIRECTORY = 'web'",
    ] {
        assert!(manifest::cube_web_directory(text.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn asset_urls_reject_traversal_and_escape_literal_unicode() -> TestResult {
    assert_eq!(manifest::asset_url("assets/糖 image.png", false)?, "/assets/%E7%B3%96%20image.png");
    assert_eq!(manifest::asset_url("lib/index.js", true)?, "/extensions/SugarCubes/lib/index.js");
    for path in ["../x", "a/../x", "a\\x", "a?x", "a%2fx", "a//x", "a:x", "a\nx"] {
        assert!(manifest::asset_url(path, false).is_err());
    }
    Ok(())
}
fn inventory() -> FrontendSourceManifest {
    FrontendSourceManifest {
        frontend: "1".into(),
        cube: "1".into(),
        assets: Vec::new(),
        cube_paths: Vec::new(),
        script: "E:\\fixture\\main.py".into(),
        script_hash: [1; 32],
    }
}
fn asset(path: &str, id: u8, bytes: u64) -> FrontendAsset {
    FrontendAsset { path: path.into(), bytes, hash: [1; 32], identity: [id; 24] }
}
#[test]
fn duplicate_paths_and_file_identity_aliases_cannot_double_count_sources() -> TestResult {
    let mut value = inventory();
    manifest::enroll(&mut value, asset("/a.js", 1, 1), false)?;
    assert!(manifest::enroll(&mut value, asset("/a.js", 2, 1), false).is_err());
    assert!(manifest::enroll(&mut value, asset("/b.js", 1, 1), false).is_err());
    assert_eq!(value.assets.len(), 1);
    Ok(())
}
#[test]
fn asset_and_aggregate_byte_bounds_fail_before_inventory_activation() -> TestResult {
    let mut value = inventory();
    assert!(
        manifest::enroll(&mut value, asset("/too-big", 1, MAX_FRONTEND_ASSET_BYTES + 1), false)
            .is_err()
    );
    for id in 1..=8 {
        manifest::enroll(
            &mut value,
            asset(&format!("/{id}"), id, MAX_FRONTEND_ASSET_BYTES),
            false,
        )?;
    }
    assert!(manifest::enroll(&mut value, asset("/over-total", 9, 1), false).is_err());
    assert_eq!(value.assets.len(), 8);
    Ok(())
}
#[test]
fn only_enrolled_cube_javascript_is_required_in_extensions() -> TestResult {
    let mut value = inventory();
    manifest::enroll(&mut value, asset("/extensions/SugarCubes/a.js", 1, 1), true)?;
    manifest::enroll(&mut value, asset("/extensions/SugarCubes/a.png", 2, 1), true)?;
    manifest::enroll(&mut value, asset("/assets/b.js", 3, 1), false)?;
    assert_eq!(value.cube_paths, ["/extensions/SugarCubes/a.js"]);
    Ok(())
}
#[test]
fn source_locations_reject_relative_nul_and_excess_retained_capacity() {
    use std::path::PathBuf;
    let valid = || PathBuf::from("E:\\fixture");
    for invalid in
        [PathBuf::from("relative"), PathBuf::from("E:\\x\0y"), PathBuf::with_capacity(256 * 1024)]
    {
        assert!(FrontendInstallationInput::new(invalid, valid(), valid(), valid()).is_err());
    }
}
