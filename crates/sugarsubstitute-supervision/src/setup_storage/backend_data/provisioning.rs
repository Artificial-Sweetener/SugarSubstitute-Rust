// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the existing standalone-managed root and Ready attempt without changing their bytes.
use super::*;
use crate::durable_storage::files;
use serde_json::Value;

/// The installer worker's native-record protocol owns these schema-one receipts.
pub(in crate::setup_storage) fn verify(
    root: &Path,
    source: &Directory,
    context: &JobContext,
) -> io::Result<Vec<File>> {
    check(context)?;
    let managed = Directory::open(root, false)?;
    let (marker, marker_file) = read(&managed, ".substitute-managed.json")?;
    if marker
        != serde_json::json!({"schema_version":1,"product":"SugarSubstitute","kind":"managed-comfy"})
    {
        return Err(invalid("Managed ComfyUI ownership marker is incompatible"));
    }
    let attempt = Directory::open(
        source.path().parent().ok_or_else(|| invalid("Managed ComfyUI attempt missing"))?,
        false,
    )?;
    let (receipt, receipt_file) = read(&attempt, ".substitute-provisioning.json")?;
    if receipt.as_object().is_none_or(|fields| {
        fields.len() != 4
            || !fields.keys().all(|key| {
                matches!(key.as_str(), "schema_version" | "owner" | "state" | "installation")
            })
    }) {
        return Err(invalid("Unsupported managed provisioning receipt fields"));
    }
    let owner = receipt
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Managed provisioning identity missing"))?;
    if !uuid(owner)
        || receipt.get("schema_version") != Some(&serde_json::json!(1))
        || receipt.get("state").and_then(Value::as_str) != Some("ready")
    {
        return Err(invalid("Managed ComfyUI has no supported Ready provisioning receipt"));
    }
    let installed = receipt
        .get("installation")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Managed installation provenance missing"))?;
    if installed.get("id").and_then(Value::as_str) != Some(owner)
        || installed.get("sourceId").and_then(Value::as_str) != Some("standalone")
    {
        return Err(invalid("Managed provisioning receipt belongs to another source"));
    }
    let install_path = installed
        .get("installPath")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Managed installation path missing"))?;
    let recorded = Directory::open(Path::new(install_path), false)?;
    let selected = managed.child(owner)?;
    let comfy = attempt.child("ComfyUI")?;
    if recorded.identity()? != attempt.identity()?
        || selected.identity()? != attempt.identity()?
        || comfy.identity()? != source.identity()?
    {
        return Err(invalid("Managed provisioning native directory association changed"));
    }
    check(context)?;
    Ok(vec![marker_file, receipt_file])
}
fn read(directory: &Directory, name: &str) -> io::Result<(Value, File)> {
    let mut file = directory.read(name)?;
    files::single_link(&file)?;
    let value = serde_json::from_slice(&files::read_bounded(&mut file, 65536)?)
        .map_err(io::Error::other)?;
    Ok((value, file))
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
