// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Foreign names, changed identities, linked objects and unsupported journals never grant deletion.
use super::*;

#[test]
fn replaced_stage_is_preserved_even_with_identical_content() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"same bytes")?;
            let source = parent.path().join(stage.record.stage());
            let journal = parent.path().join(stage.record.journal_name());
            drop(stage);
            std::fs::rename(&source, parent.path().join("moved-original"))?;
            protection.file(&source, context)?.write_all(b"same bytes")?;
            recover(&parent, protection, context)?;
            assert_eq!(std::fs::read(&source)?, b"same bytes");
            assert!(journal.exists());
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn linked_stage_is_preserved_even_when_native_identity_matches() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"linked")?;
            let source = parent.path().join(stage.record.stage());
            let journal = parent.path().join(stage.record.journal_name());
            drop(stage);
            std::fs::hard_link(&source, parent.path().join("user-alias"))?;
            recover(&parent, protection, context)?;
            assert_eq!(std::fs::read(&source)?, b"linked");
            assert!(journal.exists());
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn unknown_future_and_corrupt_journals_and_unjournaled_files_remain_intact() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            for mode in 0..4 {
                let stage = Stage::create(&parent, protection, context)?;
                let source = parent.path().join(stage.record.stage());
                let journal = parent.path().join(stage.record.journal_name());
                let mut value = serde_json::to_value(&stage.record)?;
                drop(stage);
                let bytes = match mode {
                    0 => {
                        value["schema_version"] = 2.into();
                        serde_json::to_vec(&value)?
                    }
                    1 => {
                        value["parent"] = serde_json::json!([1, 2, 3]);
                        serde_json::to_vec(&value)?
                    }
                    2 => {
                        value["id"] = "../authored".into();
                        serde_json::to_vec(&value)?
                    }
                    _ => b"interrupted journal".to_vec(),
                };
                std::fs::write(&journal, &bytes)?;
                recover(&parent, protection, context)?;
                assert!(source.exists());
                assert_eq!(std::fs::read(&journal)?, bytes);
            }
            let unjournaled = parent.path().join(Record::stage_name(&"f".repeat(32)));
            std::fs::write(&unjournaled, b"unknown")?;
            recover(&parent, protection, context)?;
            assert_eq!(std::fs::read(unjournaled)?, b"unknown");
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn journal_name_accepts_only_exact_canonical_attempt_leaves() {
    assert_eq!(
        Record::journal_id(&format!(".substitute-publication-{}.json", "a".repeat(32))),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    for name in [
        ".substitute-publication-../file.json",
        ".substitute-publication-A0000000000000000000000000000000.json",
        ".substitute-publication-a.json",
        "arbitrary.json",
    ] {
        assert!(Record::journal_id(name).is_none());
    }
}
