// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initial signed reservation remains recoverable at every directory-publication boundary.
use super::*;

#[test]
fn closing_after_reservation_reopens_the_same_initial_generation() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let runtime = runtime(temporary.path())?;
        let prepared = import(temporary.path(), context)?.prepare(|| NOW, context)?;
        let store = SelectionStore::open(&temporary.path().join("native"), context)?;
        let before = store.load(context)?.ok_or("No reservation")?;
        let generation = before.candidate().ok_or("No candidate")?.generation().clone();
        drop(prepared);
        assert!(inspect_installation(&temporary.path().join("native"), context)?.is_none());
        let candidate = import(temporary.path(), context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        assert_eq!(
            candidate.directory().file_name().and_then(|n| n.to_str()),
            Some(generation.id.as_str())
        );
        assert_eq!(std::fs::read_dir(temporary.path().join("native/releases"))?.count(), 1);
        assert!(store.load(context)?.ok_or("No state")?.accepted().is_none());
        Ok(())
    })
}

#[test]
fn interruption_before_canonical_generation_publication_preserves_resumable_intent() -> TestResult {
    for during_staging in [false, true] {
        interrupted_reservation(during_staging)?;
    }
    Ok(())
}

fn interrupted_reservation(during_staging: bool) -> TestResult {
    let temporary = tempfile::tempdir()?;
    let runtime = runtime(temporary.path())?;
    let path = temporary.path().to_owned();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        let imported = import(&path, context)?;
        let observed = path.join("native");
        clock.observe_checks(move || {
            let Some(record) =
                std::fs::read(observed.join(".selection-state/.substitute-selection.json")).ok()
            else {
                return false;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&record) else {
                return false;
            };
            if value["candidate"]["phase"] != "staging" {
                return false;
            }
            if !during_staging {
                return true;
            }
            std::fs::read_dir(observed.join("releases")).ok().is_some_and(|entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with(".generation-init-"))
                })
            })
        })?;
        assert!(imported.prepare(|| NOW, context).is_err());
        Ok(())
    })?;
    let path = temporary.path().to_owned();
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let store = SelectionStore::open(&path.join("native"), context)?;
        let before = store.load(context)?.ok_or("No reservation")?;
        let generation = before.candidate().ok_or("No candidate")?.generation().clone();
        let canonical = path.join("native/releases").join(generation.id.as_str());
        assert!(!canonical.exists());
        assert!(inspect_installation(&path.join("native"), context)?.is_none());
        let abandoned = if during_staging {
            std::fs::read_dir(path.join("native/releases"))?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<io::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let candidate = import(&path, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        assert_eq!(candidate.directory(), canonical);
        for orphan in abandoned {
            assert!(orphan.is_dir());
        }
        assert!(store.load(context)?.ok_or("No state")?.accepted().is_none());
        Ok(())
    })
}

#[test]
fn unknown_canonical_files_and_missing_prepared_generations_are_never_reinitialized() -> TestResult
{
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for complete in [false, true] {
            let temporary = tempfile::tempdir()?;
            let runtime = runtime(temporary.path())?;
            let prepared = import(temporary.path(), context)?.prepare(|| NOW, context)?;
            if complete {
                drop(prepared.complete(&runtime, || NOW, context)?);
            } else {
                drop(prepared);
            }
            let root = temporary.path().join("native");
            let store = SelectionStore::open(&root, context)?;
            let before = store.load(context)?.ok_or("No state")?;
            let canonical = root
                .join("releases")
                .join(before.candidate().ok_or("No candidate")?.generation().id.as_str());
            let preserved = temporary.path().join("preserved-generation");
            std::fs::rename(&canonical, &preserved)?;
            if !complete {
                std::fs::write(&canonical, b"unknown user file")?;
            }
            assert!(
                import(temporary.path(), context)
                    .and_then(|imported| imported.prepare(|| NOW, context).map_err(Into::into))
                    .is_err()
            );
            assert_eq!(store.load(context)?, Some(before));
            assert!(preserved.is_dir());
            if complete {
                assert!(!canonical.exists());
            } else {
                assert_eq!(std::fs::read(canonical)?, b"unknown user file");
            }
        }
        Ok(())
    })
}
