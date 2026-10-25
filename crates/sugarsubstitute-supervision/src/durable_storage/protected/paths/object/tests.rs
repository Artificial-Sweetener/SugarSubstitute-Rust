// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual combined producer and checked closure on an admitted central worker.
use super::*;
use crate::native_claims::files::test_support::{TestResult, admitted};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Actual temporary filesystem shapes supplied to the production native boundary.
#[derive(Clone, Copy)]
enum Fixture {
    Regular,
    HardLinked,
    Directory,
}

/// Own every temporary name and retain rejected outputs until the same worker closes them.
fn inspect_fixture(kind: Fixture) -> TestResult {
    admitted(move |context| {
        let directory = std::env::temp_dir().join(format!(
            "sugarsubstitute-regular-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&directory).map_err(|e| ClaimError::new("fixture_directory", e))?;
        let path = directory.join("selected");
        let alias = directory.join("alias");
        if matches!(kind, Fixture::Directory) {
            std::fs::create_dir(&path).map_err(|e| ClaimError::new("fixture_child", e))?;
        } else {
            std::fs::write(&path, b"retained frontend bytes")
                .map_err(|e| ClaimError::new("fixture_contents", e))?;
        }
        if matches!(kind, Fixture::HardLinked) {
            std::fs::hard_link(&path, &alias)
                .map_err(|e| ClaimError::new("fixture_hard_link", e))?;
        }
        let budget = files::Budget::Protected(Arc::new(
            context
                .memory()
                .claim(crate::durable_storage::protected::STORAGE_BYTES)
                .map_err(|e| ClaimError::new("fixture_memory", e))?,
        ));
        let mut handles = Vec::with_capacity(4);
        let mut phase = NativePhase::ordinary(&budget)?;
        let result = (|| {
            let opened = open_regular_retained(&path, budget.clone(), &mut handles, &mut phase);
            if !matches!(kind, Fixture::Regular) {
                let expected = if matches!(kind, Fixture::HardLinked) {
                    "claim_file_shape"
                } else {
                    "path_object_type"
                };
                match opened {
                    Err(cause) if cause.operation() == expected => {}
                    Err(cause) => return Err(cause),
                    Ok(_) => return Err(ClaimError::invalid("fixture_invalid_file_accepted")),
                }
                if handles.len() != 1 || !handles[0].is_armed() {
                    return Err(ClaimError::invalid("fixture_rejected_output_lost"));
                }
                return Ok(());
            }
            let index = opened?;
            let canonical = super::super::canonical_path(&handles[index], &mut phase)?;
            let identity = super::super::file_identity(&handles[index], &mut phase)?;
            super::super::verify_pinned(
                &canonical,
                identity,
                &mut handles,
                false,
                budget.clone(),
                &mut phase,
            )?;
            if handles.len() != 2
                || !handles.iter().all(Handle::is_armed)
                || super::super::file_identity(&handles[1], &mut phase)? != identity
            {
                return Err(ClaimError::invalid("fixture_canonical_reopen_changed"));
            }
            let writable = std::fs::OpenOptions::new().write(true).open(&path);
            if writable.is_ok() {
                return Err(ClaimError::invalid("fixture_write_sharing_admitted"));
            }
            Ok(())
        })();
        let mut result = result;
        for handle in handles.iter_mut().rev() {
            result = ClaimError::finish(result, handle.close());
        }
        drop(handles);
        if matches!(kind, Fixture::HardLinked) {
            result = ClaimError::finish(
                result,
                std::fs::remove_file(alias).map_err(|e| ClaimError::new("fixture_remove_alias", e)),
            );
        }
        let removed = if matches!(kind, Fixture::Directory) {
            std::fs::remove_dir(&path)
        } else {
            std::fs::remove_file(&path)
        };
        result = ClaimError::finish(
            result,
            removed.map_err(|e| ClaimError::new("fixture_remove_selected", e)),
        );
        ClaimError::finish(
            result,
            std::fs::remove_dir(directory)
                .map_err(|e| ClaimError::new("fixture_remove_directory", e)),
        )
    })
}

/// A genuine regular file retains both physical identity pins and denies a competing writer.
#[test]
fn regular_source_open_keeps_strict_identity_reopen_and_write_exclusion_until_checked_close()
-> TestResult {
    inspect_fixture(Fixture::Regular)
}

/// A real second hard link rejects before publication while native output stays explicitly owned.
#[test]
fn regular_source_open_rejects_hard_links_and_retains_the_original_native_output_until_close()
-> TestResult {
    inspect_fixture(Fixture::HardLinked)
}

/// A real directory cannot satisfy the strict regular-file contract or escape checked closure.
#[test]
fn regular_source_open_rejects_directories_and_retains_the_original_native_output_until_close()
-> TestResult {
    inspect_fixture(Fixture::Directory)
}
