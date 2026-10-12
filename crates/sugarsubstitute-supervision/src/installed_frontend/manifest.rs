// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate installed metadata and retain one complete bounded runtime-file inventory.
use super::*;

pub(super) fn distribution_version(bytes: &[u8]) -> Result<String, FrontendSourceError> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| FrontendSourceError::new("metadata_utf8", e))?;
    let mut name = None;
    let mut version = None;
    for line in text.lines().take_while(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("Name: ")
            && name.replace(value).is_some()
        {
            return Err(FrontendSourceError::invalid("metadata_duplicate_name"));
        }
        if let Some(value) = line.strip_prefix("Version: ")
            && version.replace(value).is_some()
        {
            return Err(FrontendSourceError::invalid("metadata_duplicate_version"));
        }
    }
    if name != Some("comfyui_frontend_package") && name != Some("comfyui-frontend-package") {
        return Err(FrontendSourceError::invalid("metadata_distribution"));
    }
    version
        .map(validate_version)
        .transpose()?
        .ok_or_else(|| FrontendSourceError::invalid("metadata_version"))
}

fn validate_version(value: &str) -> Result<String, FrontendSourceError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+_".contains(&b))
    {
        return Err(FrontendSourceError::invalid("installed_version_shape"));
    }
    Ok(value.to_owned())
}

pub(super) fn cube_version(bytes: &[u8]) -> Result<String, FrontendSourceError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| FrontendSourceError::new("cube_metadata_utf8", e))?;
    let value = text
        .parse::<toml::Value>()
        .map_err(|e| FrontendSourceError::new("cube_metadata_toml", e))?;
    let project = value
        .get("project")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| FrontendSourceError::invalid("cube_project_metadata"))?;
    if project.get("name").and_then(toml::Value::as_str) != Some("SugarCubes") {
        return Err(FrontendSourceError::invalid("cube_project_name"));
    }
    validate_version(
        project
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| FrontendSourceError::invalid("cube_project_version"))?,
    )
}

/// Accept the supported literal WEB_DIRECTORY declaration without executing Python.
pub(super) fn cube_web_directory(bytes: &[u8]) -> Result<(), FrontendSourceError> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| FrontendSourceError::new("cube_module_utf8", e))?;
    let declarations: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            line.strip_prefix("WEB_DIRECTORY")
                .and_then(|rest| rest.trim_start().strip_prefix('=').map(str::trim))
        })
        .collect();
    if declarations.len() != 1
        || !matches!(declarations[0], "\"web\"" | "'web'" | "\"./web\"" | "'./web'")
    {
        return Err(FrontendSourceError::invalid("cube_web_directory_declaration"));
    }
    Ok(())
}

pub(super) fn asset_url(relative: &str, cube: bool) -> Result<String, FrontendSourceError> {
    if relative.is_empty()
        || relative.len() > 1024
        || relative.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || relative.chars().any(|c| c.is_control() || "\\?#%:".contains(c))
    {
        return Err(FrontendSourceError::invalid("asset_relative_path"));
    }
    let prefix = if cube { "/extensions/SugarCubes/" } else { "/" };
    let mut url = String::with_capacity(prefix.len() + relative.len());
    url.push_str(prefix);
    for byte in relative.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut url, "%{byte:02X}")
                .map_err(|e| FrontendSourceError::new("asset_url_encoding", e))?;
        }
    }
    if url.len() > 3072 {
        return Err(FrontendSourceError::invalid("asset_url_bound"));
    }
    Ok(url)
}

pub(super) fn enroll(
    manifest: &mut FrontendSourceManifest,
    asset: FrontendAsset,
    cube: bool,
) -> Result<(), FrontendSourceError> {
    let retained_path_bytes = manifest.assets.iter().map(|a| a.path.capacity()).sum::<usize>()
        + manifest.cube_paths.iter().map(String::capacity).sum::<usize>()
        + asset.path.capacity() * if cube && asset.path.ends_with(".js") { 2 } else { 1 };
    if retained_path_bytes > 12 * 1024 * 1024
        || asset.bytes > MAX_FRONTEND_ASSET_BYTES
        || manifest.assets.len() >= MAX_FRONTEND_ASSETS
        || manifest
            .assets
            .iter()
            .map(|a| a.bytes)
            .sum::<u64>()
            .checked_add(asset.bytes)
            .is_none_or(|total| total > MAX_FRONTEND_TOTAL_BYTES)
        || manifest.assets.iter().any(|a| a.path == asset.path || a.identity == asset.identity)
    {
        return Err(FrontendSourceError::invalid("asset_inventory_bound_or_alias"));
    }
    if cube && asset.path.ends_with(".js") {
        manifest.cube_paths.push(asset.path.clone());
    }
    manifest.assets.push(asset);
    Ok(())
}
