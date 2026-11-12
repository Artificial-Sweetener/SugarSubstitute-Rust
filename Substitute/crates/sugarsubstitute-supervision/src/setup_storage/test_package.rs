// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Complete inert distributions for real extraction and maintenance boundary tests.
pub(crate) mod maintenance;
mod trial;
pub(crate) mod updater;
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
pub(crate) use trial::trial_transfer;

/// Build all required distribution roles; none of these fixture bytes are executed.
pub(crate) fn distribution() -> io::Result<(Vec<u8>, Vec<u8>)> {
    distribution_with_contract(Some(
        sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT,
    ))
}

/// Produce historical or future declarations without executing any fixture application bytes.
pub(crate) fn distribution_with_contract(
    contract: Option<&[u8]>,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    distribution_with_launcher(contract, None)
}

/// Pair independently produced application and launcher declarations in an inert distribution.
pub(crate) fn distribution_with_launcher(
    contract: Option<&[u8]>,
    launcher: Option<&[u8]>,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    distribution_with_roles(contract, launcher, None)
}

/// Supply optional independent role declarations while retaining the historical package shape.
pub(crate) fn distribution_with_roles(
    contract: Option<&[u8]>,
    launcher: Option<&[u8]>,
    updater: Option<&[u8]>,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    distribution_with_installation(contract, launcher, updater, None)
}

/// Add durable-choice capabilities explicitly; historical role fixtures keep their old metadata.
pub(crate) fn distribution_with_installation(
    contract: Option<&[u8]>,
    launcher: Option<&[u8]>,
    updater: Option<&[u8]>,
    installation: Option<&[u8]>,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    for name in [
        "app/sugarsubstitute.exe",
        "app/sugarsubstitute-release-v1.manifest",
        "app/sugarsubstitute-release-v1.sig",
        "app/crashpad_handler.exe",
        "app/sugarsubstitute_crashpad_client.dll",
        "electron/electron.exe",
        "worker/package.json",
        "worker/out/main/index.cjs",
        "sugarcubes/__init__.py",
        "sugarcubes/pyproject.toml",
        "sugarcubes/prestartup_script.py",
        "sugarcubes/load_model_roots.py",
        "sugarcubes/sugarcubes_model_roots/__init__.py",
    ] {
        let bytes = format!("inert fixture payload: {name}");
        entries.push(serde_json::json!({"path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes.as_bytes()))}));
        gzip.write_all(bytes.as_bytes())?;
    }
    if let Some(bytes) = contract {
        entries.push(serde_json::json!({
            "path":format!("app/{}",sugarsubstitute_contracts::application_launch::CONTRACT_FILE),
            "bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),
        }));
        gzip.write_all(bytes)?;
    }
    if let Some(launcher) = launcher {
        for (name, bytes) in [
            ("app/sugarsubstitute-launcher.exe", b"inert launcher".as_slice()),
            ("app/sugarsubstitute_launcher.pdb", b"inert launcher symbols".as_slice()),
            ("app/sugarsubstitute-launcher.json", launcher),
        ] {
            entries.push(serde_json::json!({
                "path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),
            }));
            gzip.write_all(bytes)?;
        }
    }
    if let Some(updater) = updater {
        for (name, bytes) in [
            ("app/sugarsubstitute-updater.exe", b"inert updater".as_slice()),
            ("app/sugarsubstitute_updater.pdb", b"inert updater symbols".as_slice()),
            ("app/sugarsubstitute-updater.json", updater),
        ] {
            entries.push(serde_json::json!({
                "path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),
            }));
            gzip.write_all(bytes)?;
        }
    }
    if let Some(bytes) = installation {
        entries.push(serde_json::json!({
            "path":format!("app/{}",sugarsubstitute_contracts::application_launch::installation::CONTRACT_FILE),
            "bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),
        }));
        gzip.write_all(bytes)?;
    }
    let manifest = serde_json::to_vec(
        &serde_json::json!({"schema_version":1,"release_id":"local-package","files":entries}),
    )?;
    Ok((manifest, gzip.finish()?))
}
/// Serialize the documented common archive envelope without a platform executable prefix.
pub(crate) fn seal(manifest: &[u8], compressed: &[u8]) -> Vec<u8> {
    let mut archive = compressed.to_vec();
    archive.extend_from_slice(manifest);
    archive.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
    archive.extend_from_slice(&(manifest.len() as u64).to_le_bytes());
    archive.extend_from_slice(b"SSSETUP1");
    archive
}

pub(crate) mod repository;
