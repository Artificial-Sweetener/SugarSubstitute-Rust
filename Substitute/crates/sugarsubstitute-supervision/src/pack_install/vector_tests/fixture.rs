// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build archives and pack folders from the shared install vectors in owned temp folders.

use serde_json::Value;
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;

pub(super) type Failure = Box<dyn Error + Send + Sync>;

/// Return a required field of a vector, failing the test on a malformed vector.
pub(super) fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Failure> {
    value.get(key).ok_or_else(|| format!("vector lacks {key}").into())
}

/// Return a required text field.
pub(super) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Failure> {
    field(value, key)?.as_str().ok_or_else(|| format!("vector {key} is not text").into())
}

/// Return a required list field.
pub(super) fn list<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, Failure> {
    field(value, key)?.as_array().ok_or_else(|| format!("vector {key} is not a list").into())
}

/// Return a list of texts.
pub(super) fn texts(value: &Value, key: &str) -> Result<Vec<String>, Failure> {
    list(value, key)?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| "not text".into()))
        .collect()
}

/// Write a zip whose members follow the vector description, in order, uncompressed.
///
/// A member's Unix mode is stored in the external attributes exactly as SugarCubes' fixture
/// stores it, so link and device members are what the reader sees.
pub(super) fn zip_bytes(members: &[Value]) -> Result<Vec<u8>, Failure> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    for member in members {
        let name = text(member, "name")?.as_bytes();
        let data = member.get("text").and_then(Value::as_str).unwrap_or_default().as_bytes();
        let kind = text(member, "kind")?;
        let mode = match member.get("unixMode").and_then(Value::as_str) {
            Some(octal) => u32::from_str_radix(octal, 8)? << 16,
            None if kind == "directory" => (0o40_755 << 16) | 0x10,
            None => 0o100_644 << 16,
        };
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let offset = u32::try_from(local.len())?;
        let size = u32::try_from(data.len())?;
        let name_length = u16::try_from(name.len())?;
        let header = |signature: u32, central_header: bool| {
            let mut bytes = signature.to_le_bytes().to_vec();
            if central_header {
                bytes.extend((3_u16 << 8 | 20).to_le_bytes());
            }
            for value in [20_u16, 0x800, 0, 0, 0x21] {
                bytes.extend(value.to_le_bytes());
            }
            for value in [crc.sum(), size, size] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(name_length.to_le_bytes());
            bytes.extend(0_u16.to_le_bytes());
            bytes
        };
        local.extend(header(0x0403_4b50, false));
        local.extend(name);
        local.extend(data);
        central.extend(header(0x0201_4b50, true));
        for value in [0_u16, 0, 0] {
            central.extend(value.to_le_bytes());
        }
        central.extend(mode.to_le_bytes());
        central.extend(offset.to_le_bytes());
        central.extend(name);
    }
    let count = u16::try_from(members.len())?;
    let central_offset = u32::try_from(local.len())?;
    let central_size = u32::try_from(central.len())?;
    let mut bytes = local;
    bytes.extend(central);
    bytes.extend(0x0605_4b50_u32.to_le_bytes());
    for value in [0_u16, 0, count, count] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(central_size.to_le_bytes());
    bytes.extend(central_offset.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    Ok(bytes)
}

/// Return archive members for a map of plain files.
pub(super) fn file_members(files: &BTreeMap<String, String>) -> Vec<Value> {
    files
        .iter()
        .map(|(name, text)| serde_json::json!({"name": name, "kind": "file", "text": text}))
        .collect()
}

/// Create each file of a folder description under `root`.
pub(super) fn write_files(root: &Path, files: &Value) -> Result<(), Failure> {
    for (relative, content) in files.as_object().ok_or("files is not a map")? {
        let destination = root.join(relative.replace('/', "\\"));
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(destination, content.as_str().ok_or("file text is not text")?)?;
    }
    Ok(())
}

/// Apply edits to an installed folder: text replaces a file, null deletes it.
pub(super) fn apply_edits(pack: &Path, edits: &Value) -> Result<(), Failure> {
    for (relative, content) in edits.as_object().ok_or("edits is not a map")? {
        let target = pack.join(relative.replace('/', "\\"));
        match content.as_str() {
            Some(text) => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(target, text)?;
            }
            None => std::fs::remove_file(target)?,
        }
    }
    Ok(())
}

/// Build an installed pack folder from an ownership or replacement vector.
pub(super) fn build_folder(
    root: &Path,
    folder: &Value,
    context: &JobContext,
) -> Result<PathBuf, Failure> {
    use crate::durable_storage::files::reparse_fixture::junction;
    let pack = root.join("custom_nodes").join("SugarCubes");
    let files = field(folder, "files")?;
    let real = if folder.get("link").and_then(Value::as_bool) == Some(true) {
        root.join("elsewhere").join("SugarCubes")
    } else {
        pack.clone()
    };
    std::fs::create_dir_all(&real)?;
    write_files(&real, files)?;
    if let Some(tracking) = folder.get("tracking").and_then(Value::as_str) {
        std::fs::write(real.join(".tracking"), tracking)?;
    }
    if real != pack {
        std::fs::create_dir_all(root.join("custom_nodes"))?;
        junction(&pack, &real, context)?;
        return Ok(pack);
    }
    match folder.get("git").and_then(Value::as_str) {
        Some("directory") => std::fs::create_dir(pack.join(".git"))?,
        Some("file") => std::fs::write(pack.join(".git"), "gitdir: elsewhere")?,
        _ => {}
    }
    for linked in folder.get("linkedDirectories").and_then(Value::as_array).into_iter().flatten() {
        let name = linked.as_str().ok_or("linked directory is not text")?;
        let target = root.join("outside").join(name);
        std::fs::create_dir_all(&target)?;
        junction(&pack.join(name), &target, context)?;
    }
    Ok(pack)
}

/// Return every file under `root` as forward-slash path to UTF-8 text.
pub(super) fn folder_files(root: &Path) -> Result<BTreeMap<String, String>, Failure> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for child in std::fs::read_dir(&directory)? {
            let path = child?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
                files.insert(relative, std::fs::read_to_string(&path)?);
            }
        }
    }
    Ok(files)
}

/// Return whether a path exists, without following a link.
pub(super) fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Read one file's bytes.
pub(super) fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    Ok(std::fs::read(path)?)
}
