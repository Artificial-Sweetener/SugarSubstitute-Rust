// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check actual immutable binding publication without fabricating authenticated admission.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};

fn expected(reference: EntryOriginalsReference) -> Adoption {
    Adoption {
        schema_version: 1,
        manifest: *reference.manifest_sha256(),
        request: 7,
        source_ticks: 500,
        source_frequency: 100,
        recipient_namespace: [3; 32],
        recipient_epoch: [4; 16],
        run_instance: 1,
        run_generation: 1,
    }
}

#[test]
fn exact_adoption_reopens_but_forwarded_request_cutoff_or_recipient_cannot_rebind_originals()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = protection::with(context, |owner| {
            let path = temporary.path().join("passive");
            owner.directory(&path, Mode::CreateNew, context)?;
            Directory::open(&path, false)
        })?;
        let reference = EntryOriginalsReference::new([9; 32])?;
        let expected = expected(reference);
        let first = bind_record(&parent, reference, &expected, context)?;
        let path = parent
            .path()
            .join("adoptions-v1")
            .join(format!("{}.json", record_file::hex(reference.manifest_sha256())));
        let original = std::fs::read(&path)?;
        assert!(std::fs::write(&path, b"changed binding").is_err());
        drop(first);
        for field in 0..5 {
            let mut foreign = expected.clone();
            match field {
                0 => foreign.request += 1,
                1 => foreign.source_ticks += 1,
                2 => foreign.recipient_namespace = [8; 32],
                3 => foreign.recipient_epoch = [8; 16],
                _ => foreign.run_instance += 1,
            }
            assert!(bind_record(&parent, reference, &foreign, context).is_err());
            assert_eq!(std::fs::read(&path)?, original);
        }
        let replay = bind_record(&parent, reference, &expected, context)?;
        assert_eq!(std::fs::read(&path)?, original);
        drop(replay);
        Ok(())
    })
}

#[test]
fn damaged_binding_and_expired_original_context_preserve_records_without_repair() -> TestResult {
    test_worker::run_reserved_expiring(32 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let parent = protection::with(context, |owner| {
            let path = temporary.path().join("passive");
            owner.directory(&path, Mode::CreateNew, context)?;
            Directory::open(&path, false)
        })?;
        let reference = EntryOriginalsReference::new([9; 32])?;
        let expected = expected(reference);
        drop(bind_record(&parent, reference, &expected, context)?);
        let path = parent
            .path()
            .join("adoptions-v1")
            .join(format!("{}.json", record_file::hex(reference.manifest_sha256())));
        let corrupt = b"{unsupported original binding";
        std::fs::write(&path, corrupt)?;
        assert!(bind_record(&parent, reference, &expected, context).is_err());
        assert_eq!(std::fs::read(&path)?, corrupt);
        clock.expire();
        assert!(bind_record(&parent, reference, &expected, context).is_err());
        assert_eq!(std::fs::read(&path)?, corrupt);
        Ok(())
    })
}
