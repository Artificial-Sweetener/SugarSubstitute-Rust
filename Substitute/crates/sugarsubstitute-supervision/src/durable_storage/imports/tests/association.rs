// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep source-host correlation distinct from installation ownership in actual durable receipts.
use super::*;

#[test]
fn source_host_copy_is_scoped_to_exact_profile_namespace_and_correlation_without_installation_adoption()
-> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let profile = ProfileId::new("real".into())?;
        let namespace = NamespaceId::new([7; 32]);
        let association = ImportAssociation::SourceHost {
            correlation: &fixture.identity,
            profile: &profile,
            namespace,
        };
        copy_tree(
            &fixture.root,
            association,
            &fixture.source,
            &fixture.target,
            RootEntries::Except(&[".lease"]),
            context,
        )?;
        verify_completed(&fixture.root, association, &fixture.target, context)?;
        assert!(
            verify_completed(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &fixture.target,
                context
            )
            .is_err()
        );
        let other = ProfileId::new("Real".into())?;
        for changed in [
            ImportAssociation::SourceHost {
                correlation: &fixture.identity,
                profile: &other,
                namespace,
            },
            ImportAssociation::SourceHost {
                correlation: &fixture.identity,
                profile: &profile,
                namespace: NamespaceId::new([8; 32]),
            },
        ] {
            assert!(verify_completed(&fixture.root, changed, &fixture.target, context).is_err());
        }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture.receipt()?)?)?;
        assert_eq!(value["journal"]["association"]["kind"], "source_host");
        assert_eq!(value["journal"]["association"]["profile"], "real");
        assert!(value["journal"]["association"].get("installation_id").is_none());
        assert!(!fixture.root.path().join("state/user-data-imports").exists());
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        std::fs::write(fixture.target.join("document"), b"later source-host user edits")?;
        copy_tree(
            &fixture.root,
            association,
            &fixture.source,
            &fixture.target,
            RootEntries::Except(&[".lease"]),
            context,
        )?;
        assert_eq!(
            std::fs::read(fixture.target.join("document"))?,
            b"later source-host user edits"
        );
        Ok(())
    })
}

#[test]
fn unknown_association_and_empty_native_namespace_refuse_without_rewriting_originals() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let profile = ProfileId::new("real".into())?;
        let empty = ImportAssociation::SourceHost {
            correlation: &fixture.identity,
            profile: &profile,
            namespace: NamespaceId::new([0; 32]),
        };
        assert!(
            copy_tree(
                &fixture.root,
                empty,
                &fixture.source,
                &fixture.target,
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert!(!fixture.target.exists());
        assert!(!fixture.root.path().join("user/migration-originals/import-records").exists());
        fixture.copy(context)?;
        let path = fixture.receipt()?;
        let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        value["journal"]["association"]["kind"] = serde_json::json!("future_owner");
        let original = serde_json::to_vec(&value)?;
        std::fs::write(&path, &original)?;
        assert!(fixture.copy(context).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        assert_eq!(std::fs::read(fixture.target.join("document"))?, b"original document");
        Ok(())
    })
}
