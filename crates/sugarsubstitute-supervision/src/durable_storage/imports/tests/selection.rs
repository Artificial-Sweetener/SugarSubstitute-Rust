// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Positive root-file selection cannot admit discovered model libraries or unknown siblings.
use super::*;

#[test]
fn only_named_root_files_are_copied_and_later_discovered_siblings_grant_no_selection() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        std::fs::write(fixture.source.path().join("model_root.json"), b"exact selected control")?;
        let assets = fixture.source.path().join("models");
        std::fs::create_dir(&assets)?;
        std::fs::write(assets.join("weight.safetensors"), b"existing model bytes")?;
        copy_tree(
            &fixture.root,
            ImportAssociation::Installed(&fixture.identity),
            &fixture.source,
            &fixture.target,
            RootEntries::OnlyFiles(&["model_root.json"]),
            context,
        )?;
        assert_eq!(
            std::fs::read(fixture.target.join("model_root.json"))?,
            b"exact selected control"
        );
        assert_eq!(std::fs::read(assets.join("weight.safetensors"))?, b"existing model bytes");
        assert!(!fixture.target.join("models").exists());
        assert!(!fixture.target.join("document").exists());
        assert_eq!(std::fs::read_dir(&fixture.target)?.count(), 1);
        verify_completed(
            &fixture.root,
            ImportAssociation::Installed(&fixture.identity),
            &fixture.target,
            context,
        )?;
        Ok(())
    })
}
#[test]
fn a_selected_control_directory_or_link_never_becomes_an_asset_tree() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let control = fixture.source.path().join("model_root.json");
        std::fs::create_dir(&control)?;
        std::fs::write(control.join("weight"), b"authored bytes")?;
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &fixture.source,
                &fixture.target,
                RootEntries::OnlyFiles(&["model_root.json"]),
                context
            )
            .is_err()
        );
        assert!(!fixture.target.exists());
        assert_eq!(std::fs::read(control.join("weight"))?, b"authored bytes");
        Ok(())
    })
}
