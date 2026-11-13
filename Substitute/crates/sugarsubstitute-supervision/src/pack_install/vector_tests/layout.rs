// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the archive, ownership and replacement vectors through real folders.

use super::fixture::{build_folder, exists, folder_files, zip_bytes};
use super::*;
use crate::setup_storage::test_worker;

#[test]
fn archive_vectors() -> TestResult {
    for vector in drive("archives")? {
        let name = text(&vector, "name")?;
        let members = list(&vector, "members")?;
        let bytes = zip_bytes(members)?;
        let expect = field(&vector, "expect")?;
        let temporary = tempfile::tempdir()?;
        let out = PackTree::at(&temporary.path().join("out"))?;
        let extracted = archive::extract_registry_archive(&bytes, &out, ArchiveLimits::default());
        if let Some(error) = expect.get("error").and_then(Value::as_str) {
            match extracted {
                Err(cause) => assert_eq!(code(&cause), error, "{name}"),
                Ok(_) => return Err(format!("{name}: accepted").into()),
            }
            assert!(!exists(&temporary.path().join("out")), "{name}: wrote before refusing");
            continue;
        }
        let extracted = extracted?;
        let entries: Vec<String> =
            extracted.entries.iter().map(|entry| entry.text().to_owned()).collect();
        assert_eq!(entries, texts(expect, "entries")?, "{name}");
        assert_eq!(list_registry_archive(&bytes, ArchiveLimits::default())?, extracted.entries);
        for member in members
            .iter()
            .filter(|member| member.get("kind").and_then(Value::as_str) == Some("file"))
        {
            let content = member.get("text").and_then(Value::as_str).unwrap_or_default();
            assert_eq!(
                extracted.file_digests.get(text(member, "name")?),
                Some(&sha256(content.as_bytes())),
                "{name}"
            );
        }
    }
    Ok(())
}

#[test]
fn archive_limit_vectors() -> TestResult {
    for vector in drive("archiveLimits")? {
        let name = text(&vector, "name")?;
        let limits = field(&vector, "limits")?;
        let number = |key: &str| {
            field(limits, key).and_then(|value| value.as_u64().ok_or_else(|| "not a number".into()))
        };
        let limits = ArchiveLimits {
            max_entries: usize::try_from(number("maxEntries")?)?,
            max_total_bytes: number("maxTotalBytes")?,
            max_file_bytes: number("maxFileBytes")?,
        };
        let bytes = zip_bytes(list(&vector, "members")?)?;
        match list_registry_archive(&bytes, limits) {
            Err(cause) => {
                assert_eq!(code(&cause), text(field(&vector, "expect")?, "error")?, "{name}")
            }
            Ok(_) => return Err(format!("{name}: accepted").into()),
        }
    }
    Ok(())
}

#[test]
fn ownership_vectors() -> TestResult {
    test_worker::run(|context, _| {
        for vector in drive("ownership")? {
            let name = text(&vector, "name")?;
            let temporary = tempfile::tempdir()?;
            let pack = build_folder(temporary.path(), field(&vector, "folder")?, context)?;
            let ownership = PackFolder::at(&pack)?.ownership()?;
            let expect = field(&vector, "expect")?;
            match ownership {
                Ownership::Owned(identity) => {
                    assert_eq!(field(expect, "owned")?.as_bool(), Some(true), "{name}");
                    assert_eq!(identity.version.to_string(), text(expect, "version")?, "{name}");
                }
                Ownership::NotOwned(reason) => {
                    assert_eq!(field(expect, "owned")?.as_bool(), Some(false), "{name}");
                    assert_eq!(reason.as_str(), text(expect, "reason")?, "{name}");
                }
            }
        }
        Ok(())
    })
}

#[test]
fn replacement_vectors() -> TestResult {
    test_worker::run(|context, _| {
        for vector in drive("replacements")? {
            let name = text(&vector, "name")?;
            let temporary = tempfile::tempdir()?;
            let pack = build_folder(temporary.path(), field(&vector, "installed")?, context)?;
            let target = PackTree::at(&pack)?;
            let ownership::PackOwnership::Owned(owned) = ownership::inspect_pack_folder(&target)?
            else {
                return Err(format!("{name}: installed folder is not owned").into());
            };
            let staged = PackTree::at(&temporary.path().join("staged"))?;
            let extracted = archive::extract_registry_archive(
                &zip_bytes(list(&vector, "release")?)?,
                &staged,
                ArchiveLimits::default(),
            )?;
            let transaction = replacement::TrackedReplacement::new(PackTree::at(
                &temporary.path().join("transaction"),
            )?);
            transaction.apply(&replacement::ReplacementPlan {
                target: &target,
                source: &staged,
                old_entries: &owned.entries,
                new_entries: &extracted.entries,
                from_version: "1.0.0",
                to_version: "1.1.0",
            })?;
            let expect = field(&vector, "expect")?;
            let mut files = folder_files(&pack)?;
            let tracking = files.remove(TRACKING_FILE_NAME).ok_or("no .tracking")?;
            files.remove(DIGEST_RECORD_NAME).ok_or("no digest record")?;
            let expected: BTreeMap<String, String> =
                serde_json::from_value(field(expect, "files")?.clone())?;
            assert_eq!(files, expected, "{name}");
            for absent in texts(expect, "absent")? {
                assert!(!exists(&pack.join(absent.replace('/', "\\"))), "{name}: {absent}");
            }
            let lines: Vec<&str> = tracking.lines().filter(|line| !line.is_empty()).collect();
            assert_eq!(lines, texts(expect, "trackingEntries")?, "{name}");
            let recorded = ownership::read_digest_record(&target, "1.1.0", &extracted.entries)?;
            assert_eq!(recorded, Some(extracted.file_digests.clone()), "{name}");
        }
        Ok(())
    })
}
