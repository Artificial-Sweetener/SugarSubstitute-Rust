// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual directory-junction rejection preserves both original and unrelated target bytes.
use super::*;
use crate::durable_storage::files::reparse_fixture;
use std::os::windows::fs::MetadataExt;

#[test]
fn real_source_and_destination_junctions_never_admit_or_modify_their_targets() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let authored = fixture.root.path().join("other-authored");
        std::fs::create_dir(&authored)?;
        std::fs::write(authored.join("keep"), b"unrelated original")?;
        let source_link = fixture.source.path().join("linked");
        reparse_fixture::junction(&source_link, &authored, context)?;
        assert_ne!(std::fs::symlink_metadata(&source_link)?.file_attributes() & 0x400, 0);
        assert!(fixture.copy(context).is_err());
        assert!(!fixture.target.exists());
        assert_eq!(std::fs::read(authored.join("keep"))?, b"unrelated original");

        let destination_link = fixture.root.path().join("user").join("redirected");
        reparse_fixture::junction(&destination_link, &authored, context)?;
        let empty = Directory::open(&fixture.root.path().join("empty-source"), true)?;
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &empty,
                &destination_link.join("escaped"),
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &empty,
                &destination_link,
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert!(!authored.join("escaped").exists());
        assert_eq!(std::fs::read(authored.join("keep"))?, b"unrelated original");
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        Ok(())
    })
}
