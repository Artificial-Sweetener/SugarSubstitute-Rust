// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original records and unknown aliases while qualifying real reusable-stage publication.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};

/// Create isolated native protected fixture bytes using the same security and checked-close owner.
fn seed(parent: &Directory, name: &str, bytes: &[u8], context: &JobContext) -> io::Result<()> {
    protection::with(context, |protection| {
        let mut file = protection.file(&parent.path().join(name), context)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        protection.close_file(file)
    })
}

/// A known abandoned stage is reusable, while create-only collision preserves complete final bytes.
#[test]
fn reusable_stage_flushes_and_closes_before_replacement_and_preserves_create_only_collisions()
-> TestResult {
    worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        seed(&parent, "record", b"original", context)?;
        seed(&parent, "record.next", b"abandoned", context)?;
        publish_reusable(&parent, "record", "record.next", b"replacement", 64, true, context)?;
        assert_eq!(std::fs::read(root.path().join("record"))?, b"replacement");
        assert!(!root.path().join("record.next").try_exists()?);
        assert!(
            publish_reusable(&parent, "record", "record.next", b"attempt", 64, false, context)
                .is_err()
        );
        assert_eq!(std::fs::read(root.path().join("record"))?, b"replacement");
        assert_eq!(std::fs::read(root.path().join("record.next"))?, b"attempt");
        Ok(())
    })
}

/// Hard-linked stage bytes and their authored alias remain unchanged on rejected publication.
#[test]
fn hard_linked_reusable_stage_is_rejected_before_truncation_or_target_replacement() -> TestResult {
    worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        seed(&parent, "record", b"original", context)?;
        seed(&parent, "record.next", b"preserved", context)?;
        std::fs::hard_link(root.path().join("record.next"), root.path().join("authored"))?;
        assert!(
            publish_reusable(&parent, "record", "record.next", b"replacement", 64, true, context)
                .is_err()
        );
        assert_eq!(std::fs::read(root.path().join("record"))?, b"original");
        assert_eq!(std::fs::read(root.path().join("record.next"))?, b"preserved");
        assert_eq!(std::fs::read(root.path().join("authored"))?, b"preserved");
        Ok(())
    })
}

/// Native checked closure still runs after expiry; complete new bytes stay staged and unactivated.
#[test]
fn expired_publication_keeps_original_target_and_closed_complete_stage() -> TestResult {
    worker::run_reserved_expiring(8 * 1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        seed(&parent, "record", b"original", context)?;
        let staged = root.path().join("record.next");
        let observed = staged.clone();
        clock.observe_checks(move || {
            std::fs::read(&observed).is_ok_and(|bytes| bytes == b"replacement")
        })?;
        assert!(
            publish_reusable(&parent, "record", "record.next", b"replacement", 64, true, context)
                .is_err()
        );
        assert_eq!(std::fs::read(root.path().join("record"))?, b"original");
        assert_eq!(std::fs::read(staged)?, b"replacement");
        Ok(())
    })
}
