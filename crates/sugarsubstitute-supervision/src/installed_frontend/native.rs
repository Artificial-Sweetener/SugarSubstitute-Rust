// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin local source files, hash their complete bytes and retain enumeration handles before use.
use super::{
    state::{Inspection, SourcePort, checkpoint},
    *,
};
use crate::{native_claims::files::Budget, protected_storage::paths};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::{
    Win32::{
        Foundation::{ERROR_NO_MORE_FILES, HANDLE},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};
pub(super) struct SystemPort;
impl SourcePort for SystemPort {
    fn inspect(
        &self,
        input: &FrontendInstallationInput,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        retained
            .handles
            .try_reserve_exact(16384)
            .map_err(|e| FrontendSourceError::new("source_handle_storage", e))?;
        let mut manifest = FrontendSourceManifest {
            frontend: String::new(),
            cube: String::new(),
            assets: Vec::new(),
            cube_paths: Vec::new(),
            script: PathBuf::new(),
            script_hash: [0; 32],
        };
        let package = pin_directory(&input.package, retained, context)?;
        let cube = pin_directory(&input.cube, retained, context)?;
        if cube.file_name().and_then(|name| name.to_str()) != Some("SugarCubes") {
            return Err(FrontendSourceError::invalid("cube_extension_directory_name"));
        }
        let script = pin(&input.script, false, retained, context)?;
        manifest.script = paths::final_path(&retained.handles[script])
            .map_err(|e| FrontendSourceError::new("backend_script_path", e))?;
        let length = file_size(script, MAX_FRONTEND_ASSET_BYTES, retained, context)?;
        let mut digest = Sha256::new();
        if stream(script, retained, context, |chunk| {
            digest.update(chunk);
            Ok(())
        })? != length
        {
            return Err(FrontendSourceError::invalid("backend_script_changed_length"));
        }
        manifest.script_hash = digest.finalize().into();
        let metadata = read_metadata(&input.metadata, 64 * 1024, retained, context)?;
        manifest.frontend = super::manifest::distribution_version(&metadata)?;
        manifest.cube = super::manifest::cube_version(&read_metadata(
            &cube.join("pyproject.toml"),
            64 * 1024,
            retained,
            context,
        )?)?;
        super::manifest::cube_web_directory(&read_metadata(
            &cube.join("__init__.py"),
            1024 * 1024,
            retained,
            context,
        )?)?;
        inventory(&package.join("static"), false, &mut manifest, retained, context)?;
        inventory(&cube.join("web"), true, &mut manifest, retained, context)?;
        if manifest.assets.is_empty()
            || manifest.cube_paths.is_empty()
            || !manifest.assets.iter().any(|asset| asset.path == "/index.html")
        {
            return Err(FrontendSourceError::invalid("source_runtime_inventory"));
        }
        manifest.assets.sort_by(|a, b| a.path.cmp(&b.path));
        manifest.cube_paths.sort();
        checkpoint(context)?;
        retained.manifest = Some(Arc::new(manifest));
        Ok(())
    }
    fn close(
        &self,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        close_search(retained, context)?;
        for handle in retained.handles.iter_mut().rev() {
            checkpoint(context)?;
            handle.close().map_err(|e| FrontendSourceError::new("source_close", e))?;
            checkpoint(context)?;
        }
        retained.handles.clear();
        retained.manifest = None;
        Ok(())
    }
}
fn pin(
    path: &Path,
    directory: bool,
    retained: &mut Inspection,
    context: &JobContext,
) -> Result<usize, FrontendSourceError> {
    checkpoint(context)?;
    if retained.handles.len() > 16382 {
        return Err(FrontendSourceError::invalid("source_handle_inventory"));
    }
    let index = paths::open_retained(
        path,
        directory,
        false,
        Budget::Protected(retained.claim.clone()),
        &mut retained.handles,
    )
    .map_err(|e| FrontendSourceError::new("source_open", e))?;
    checkpoint(context)?;
    if !directory {
        paths::validate_regular(&retained.handles[index], None)
            .map_err(|e| FrontendSourceError::new("source_regular_file", e))?;
    }
    let canonical = paths::final_path(&retained.handles[index])
        .map_err(|e| FrontendSourceError::new("source_canonical", e))?;
    let identity = paths::file_identity(&retained.handles[index])
        .map_err(|e| FrontendSourceError::new("source_identity", e))?;
    checkpoint(context)?;
    paths::verify_pinned(
        &canonical,
        identity,
        &mut retained.handles,
        directory,
        Budget::Protected(retained.claim.clone()),
    )
    .map_err(|e| FrontendSourceError::new("source_verify_chain", e))?;
    checkpoint(context)?;
    Ok(index)
}
fn pin_directory(
    path: &Path,
    retained: &mut Inspection,
    context: &JobContext,
) -> Result<PathBuf, FrontendSourceError> {
    let index = pin(path, true, retained, context)?;
    let canonical = paths::final_path(&retained.handles[index])
        .map_err(|e| FrontendSourceError::new("source_directory_path", e))?;
    checkpoint(context)?;
    for ancestor in paths::ancestor_paths(&canonical)
        .map_err(|e| FrontendSourceError::new("source_directory_ancestor_shape", e))?
    {
        checkpoint(context)?;
        if retained.handles.len() >= 16384 {
            return Err(FrontendSourceError::invalid("source_handle_inventory"));
        }
        paths::open_retained(
            &ancestor,
            true,
            true,
            Budget::Protected(retained.claim.clone()),
            &mut retained.handles,
        )
        .map_err(|e| FrontendSourceError::new("source_directory_ancestors", e))?;
        checkpoint(context)?;
    }
    checkpoint(context)?;
    Ok(canonical)
}
fn read_metadata(
    path: &Path,
    maximum: u64,
    retained: &mut Inspection,
    context: &JobContext,
) -> Result<Vec<u8>, FrontendSourceError> {
    let index = pin(path, false, retained, context)?;
    let length = file_size(index, maximum, retained, context)?;
    let mut bytes = Vec::with_capacity(length as usize);
    stream(index, retained, context, |chunk| {
        bytes.extend_from_slice(chunk);
        Ok(())
    })?;
    if bytes.len() as u64 != length {
        return Err(FrontendSourceError::invalid("source_metadata_changed_length"));
    }
    Ok(bytes)
}
fn file_size(
    index: usize,
    maximum: u64,
    retained: &Inspection,
    context: &JobContext,
) -> Result<u64, FrontendSourceError> {
    checkpoint(context)?;
    let mut length = 0;
    // SAFETY: The retained regular-file owner and initialized size output live throughout the call.
    unsafe { GetFileSizeEx(retained.handles[index].raw(), &mut length) }
        .map_err(|e| FrontendSourceError::new("source_file_size", e))?;
    checkpoint(context)?;
    if length < 0 || length as u64 > maximum {
        return Err(FrontendSourceError::invalid("source_file_size_bound"));
    }
    Ok(length as u64)
}
fn stream(
    index: usize,
    retained: &Inspection,
    context: &JobContext,
    mut consume: impl FnMut(&[u8]) -> Result<(), FrontendSourceError>,
) -> Result<u64, FrontendSourceError> {
    let mut scratch = [0; FRONTEND_STREAM_BYTES];
    let mut total = 0u64;
    loop {
        checkpoint(context)?;
        let mut read = 0;
        // SAFETY: Retained synchronous handle, scratch slice and initialized count remain valid.
        unsafe {
            ReadFile(retained.handles[index].raw(), Some(&mut scratch), Some(&mut read), None)
        }
        .map_err(|e| FrontendSourceError::new("source_file_read", e))?;
        checkpoint(context)?;
        if read == 0 {
            return Ok(total);
        }
        total = total
            .checked_add(u64::from(read))
            .filter(|n| *n <= MAX_FRONTEND_ASSET_BYTES)
            .ok_or_else(|| FrontendSourceError::invalid("source_stream_bound"))?;
        consume(&scratch[..read as usize])?;
    }
}
fn inventory(
    root: &Path,
    cube: bool,
    manifest: &mut FrontendSourceManifest,
    retained: &mut Inspection,
    context: &JobContext,
) -> Result<(), FrontendSourceError> {
    let root = pin_directory(root, retained, context)?;
    let mut pending = vec![PathBuf::new()];
    let mut entries = 0usize;
    let mut path_bytes = 0usize;
    while let Some(relative) = pending.pop() {
        let directory = root.join(&relative);
        if !relative.as_os_str().is_empty() {
            pin_directory(&directory, retained, context)?;
        }
        let query = paths::wide(&directory.join("*"))
            .map_err(|e| FrontendSourceError::new("source_enumeration_path", e))?;
        checkpoint(context)?;
        let mut data = WIN32_FIND_DATAW::default();
        // SAFETY: The owned wildcard path and writable result structure live throughout the call.
        let raw = unsafe { FindFirstFileW(PCWSTR(query.as_ptr()), &mut data) }
            .map_err(|e| FrontendSourceError::new("source_enumeration_start", e))?;
        retained.search = raw.0 as usize;
        loop {
            checkpoint(context)?;
            let end = data
                .cFileName
                .iter()
                .position(|unit| *unit == 0)
                .ok_or_else(|| FrontendSourceError::invalid("source_entry_name_bound"))?;
            let name = String::from_utf16(&data.cFileName[..end])
                .map_err(|e| FrontendSourceError::new("source_entry_unicode", e))?;
            if name != "." && name != ".." {
                entries += 1;
                if entries > 8192 || data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                    return Err(FrontendSourceError::invalid("source_entry_bound_or_reparse"));
                }
                let child = relative.join(&name);
                path_bytes = path_bytes
                    .checked_add(child.capacity())
                    .filter(|n| *n <= 4 * 1024 * 1024)
                    .ok_or_else(|| FrontendSourceError::invalid("source_path_storage_bound"))?;
                if data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
                    if child.components().count() > 128 {
                        return Err(FrontendSourceError::invalid("source_directory_depth"));
                    }
                    pending.push(child);
                } else if !name.ends_with(".map") {
                    let relative = child
                        .to_str()
                        .ok_or_else(|| FrontendSourceError::invalid("source_asset_unicode"))?
                        .replace('\\', "/");
                    let path = super::manifest::asset_url(&relative, cube)?;
                    let index = pin(&root.join(child), false, retained, context)?;
                    let bytes = file_size(index, MAX_FRONTEND_ASSET_BYTES, retained, context)?;
                    let mut digest = Sha256::new();
                    let observed = stream(index, retained, context, |chunk| {
                        digest.update(chunk);
                        Ok(())
                    })?;
                    if observed != bytes {
                        return Err(FrontendSourceError::invalid("source_asset_changed_length"));
                    }
                    let identity = paths::file_identity(&retained.handles[index])
                        .map_err(|e| FrontendSourceError::new("source_asset_identity", e))?;
                    checkpoint(context)?;
                    super::manifest::enroll(
                        manifest,
                        FrontendAsset { path, bytes, hash: digest.finalize().into(), identity },
                        cube,
                    )?;
                }
            }
            checkpoint(context)?;
            // SAFETY: The actual search handle is retained before this query and result storage lives through it.
            match unsafe { FindNextFileW(HANDLE(retained.search as *mut _), &mut data) } {
                Ok(()) => (),
                Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(e) => return Err(FrontendSourceError::new("source_enumeration_next", e)),
            }
        }
        close_search(retained, context)?;
    }
    Ok(())
}
fn close_search(
    retained: &mut Inspection,
    context: &JobContext,
) -> Result<(), FrontendSourceError> {
    if retained.search != 0 {
        checkpoint(context)?;
        // SAFETY: The slot uniquely owns this live search handle and is cleared only after successful close.
        unsafe { FindClose(HANDLE(retained.search as *mut _)) }
            .map_err(|e| FrontendSourceError::new("source_enumeration_close", e))?;
        retained.search = 0;
        checkpoint(context)?;
    }
    Ok(())
}
