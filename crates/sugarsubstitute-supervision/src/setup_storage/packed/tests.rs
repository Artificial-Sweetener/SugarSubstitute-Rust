// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Both trusted entry paths share extraction, rejection and native installation staging.
use super::*;
use crate::setup_storage::{
    VerifiedBundle, test_package,
    test_worker::{self as worker, TestResult},
};
use extract::extract;
mod downloaded;

fn package(data: &[u8]) -> io::Result<Manifest> {
    Manifest::decode(&serde_json::to_vec(&serde_json::json!({
        "schema_version":1,"release_id":"test","files":[{
            "path":"app/file","bytes":data.len(),"sha256":format!("{:x}",Sha256::digest(data)),
        }],
    }))?)
    .map_err(io::Error::other)
}

#[test]
fn archive_validation_rejects_traversal_aliases_and_duplicate_names() -> io::Result<()> {
    let (manifest, _) = test_package::distribution()?;
    let mut value: serde_json::Value = serde_json::from_slice(&manifest)?;
    for path in ["app/../escape", "app/NUL", "app/./file", "app/crashpad_handler.exe"] {
        value["files"][0]["path"] = path.into();
        assert!(Manifest::decode(&serde_json::to_vec(&value)?).is_err());
    }
    Ok(())
}

#[test]
fn extraction_checks_bytes_cancellation_and_create_only_publication() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    extract(&mut &b"valid"[..], &package(b"valid")?, temporary.path(), || Ok(()))?;
    assert_eq!(std::fs::read(temporary.path().join("app/file"))?, b"valid");
    assert!(extract(&mut &b"other"[..], &package(b"other")?, temporary.path(), || Ok(())).is_err());
    let other = temporary.path().join("other");
    assert!(extract(&mut &b"wrong"[..], &package(b"valid")?, &other, || Ok(())).is_err());
    let cancelled = temporary.path().join("cancelled");
    assert!(
        extract(&mut &b"valid"[..], &package(b"valid")?, &cancelled, || Err(
            io::ErrorKind::Interrupted.into()
        ))
        .is_err()
    );
    assert!(!cancelled.exists());
    Ok(())
}

#[test]
fn footer_authentication_rejects_changed_inventory_and_truncation() -> io::Result<()> {
    let manifest = b"authenticated inventory";
    let expected = format!("{:x}", Sha256::digest(manifest));
    let mut bytes = b"EXE".to_vec();
    bytes.extend(test_package::seal(manifest, b"compressed"));
    assert_eq!(inventory(&mut io::Cursor::new(&bytes), &expected)?.offset, 3);
    assert!(inventory(&mut io::Cursor::new(&bytes), "changed").is_err());
    assert!(inventory(&mut io::Cursor::new(&bytes[..10]), &expected).is_err());
    Ok(())
}

#[test]
fn installer_and_update_use_one_extractor_that_rejects_compressed_tail_and_preserves_attempts()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let workspace = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution()?;
        let expected = format!("{:x}", Sha256::digest(&manifest));
        let source = workspace.path().join("setup.exe");
        std::fs::write(&source, test_package::seal(&manifest, &compressed))?;
        let installed = materialize(&source, workspace.path(), &expected, context)?;
        let bundle = VerifiedBundle::open(&installed, &expected, context)?;
        assert_eq!(bundle.release(), "local-package");
        let sentinel = workspace.path().join("user-file");
        std::fs::write(&sentinel, b"preserve")?;
        for suffix in [&b"trailing"[..], &compressed[..]] {
            let mut changed = compressed.clone();
            changed.extend_from_slice(suffix);
            std::fs::write(&source, test_package::seal(&manifest, &changed))?;
            assert!(materialize(&source, workspace.path(), &expected, context).is_err());
        }
        assert_eq!(std::fs::read(&sentinel)?, b"preserve");
        assert!(installed.join("setup-manifest.json").is_file());
        assert_eq!(std::fs::read_dir(workspace.path().join("payloads-v1"))?.count(), 3);
        Ok(())
    })
}
