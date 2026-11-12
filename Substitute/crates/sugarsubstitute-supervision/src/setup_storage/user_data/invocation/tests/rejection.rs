// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject changed passive copies and unsupported manifests without touching original bytes.
use super::*;

#[test]
fn copied_source_change_foreign_reference_and_manifest_overflow_refuse_without_rewriting_originals()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context, false)?;
        let mut snapshot = fixture.snapshot(context)?;
        snapshot.publish(&fixture.controls, &fixture.paths, context)?;
        let reference = snapshot.reference().ok_or("Reference absent")?;
        let parent = snapshot.host.originals_directory(false, context)?;
        let expected = snapshot.host.producer.clone();
        let original = fixture.controls.startup.as_ref().ok_or("Original absent")?.bytes.clone();
        assert!(
            reading::PinnedOriginals::new(&parent, EntryOriginalsReference::new([8; 32])?).is_err()
        );
        let directory = parent
            .path()
            .join(crate::durable_storage::protected::record_file::hex(reference.manifest_sha256()));
        let manifest_bytes = std::fs::read(directory.join(manifest::MANIFEST))?;
        assert!(manifest::Manifest::decode(&vec![b' '; manifest::MAX_MANIFEST_BYTES + 1]).is_err());
        let mut future: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
        future["schema_version"] = serde_json::json!(2);
        assert!(manifest::Manifest::decode(&serde_json::to_vec(&future)?).is_err());
        snapshot.final_files = None;
        std::fs::write(directory.join("startup.toml"), b"changed passive original")?;
        let mut receiver = reading::PinnedOriginals::new(&parent, reference)?;
        assert!(receiver.load(reference, &expected, context).is_err());
        assert_eq!(std::fs::read(directory.join("startup.toml"))?, b"changed passive original");
        assert_eq!(std::fs::read(directory.join(manifest::MANIFEST))?, manifest_bytes);
        assert_eq!(std::fs::read(fixture.paths.configuration())?, original);
        Ok(())
    })
}

#[test]
fn unknown_snapshot_role_is_preserved_and_rejected_before_profile_settlement() -> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context, false)?;
        let mut snapshot = fixture.snapshot(context)?;
        snapshot.publish(&fixture.controls, &fixture.paths, context)?;
        let reference = snapshot.reference().ok_or("Reference absent")?;
        let parent = snapshot.host.originals_directory(false, context)?;
        let expected = snapshot.host.producer.clone();
        let directory = parent
            .path()
            .join(crate::durable_storage::protected::record_file::hex(reference.manifest_sha256()));
        std::fs::write(directory.join("unknown-original"), b"unknown preserved bytes")?;
        let mut receiver = reading::PinnedOriginals::new(&parent, reference)?;
        assert!(receiver.load(reference, &expected, context).is_err());
        assert_eq!(std::fs::read(directory.join("unknown-original"))?, b"unknown preserved bytes");
        assert!(
            !fixture
                .image
                .parent()
                .ok_or("Image parent absent")?
                .join("user/desktop-data/profile")
                .exists()
        );
        Ok(())
    })
}
