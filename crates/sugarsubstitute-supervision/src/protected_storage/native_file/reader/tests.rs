// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual counted same-inode reader under original symbol admission.
use super::*;
use crate::native_claims::files::test_support::{TestResult, admitted};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::SymbolNativePartition;

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn original_counted_read_view_keeps_same_inode_and_credit_through_range_failure() -> TestResult {
    admitted(|context| {
        let directory = std::env::temp_dir().join(format!(
            "sugarsubstitute-native-read-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).map_err(|e| ClaimError::new("fixture_directory", e))?;
        let path = directory.join("selected.bin");
        let prefix = b"same copied inode";
        let mut contents: Vec<u8> = (0..131_073).map(|index| (index % 251) as u8).collect();
        contents[..prefix.len()].copy_from_slice(prefix);
        let expected_digest: [u8; 32] = Sha256::digest(&contents).into();
        std::fs::write(&path, &contents).map_err(|e| ClaimError::new("fixture_contents", e))?;
        let path =
            std::fs::canonicalize(path).map_err(|e| ClaimError::new("fixture_canonical", e))?;
        let lease =
            context.native_handles().ok_or_else(|| ClaimError::invalid("fixture_native_lease"))?;
        let claim = lease
            .take_partition(SymbolNativePartition::SelectedRelease)
            .map_err(|e| ClaimError::new("fixture_partition", e))?;
        let memory = Arc::new(
            context
                .memory()
                .claim(2 * 1024 * 1024)
                .map_err(|e| ClaimError::new("fixture_memory", e))?,
        );
        let mut file = NativeFile::reserve(
            1,
            contents.len() as u64,
            FileGrant::ReadOnly,
            &mut Some((
                crate::protected_storage::native_name::NativeName::Ordinary(path.clone()),
                crate::protected_storage::native_file::NativeFileAdmission::Symbol {
                    budget: Budget::Protected(memory),
                    original: context.clone(),
                    output: claim
                        .reserve_output()
                        .map_err(|e| ClaimError::new("fixture_output", e))?,
                },
            )),
        )?;
        let result = (|| {
            let budget = file.fields()?.handle.budget();
            let mut phase = crate::protected_storage::native_phase::NativePhase::ordinary(&budget)?;
            let unopened = file.reader(&mut phase).err().map(|error| error.operation());
            file.open(None, &context, &context, &mut phase)?;
            let unobserved = file.reader(&mut phase).err().map(|error| error.operation());
            let facts = file.observe(None, &context, &mut phase)?;
            assert_eq!(facts.sha256, expected_digest);
            file.fields_mut()?.facts = Some(facts);
            let raw = file.fields()?.handle.raw();
            let mut view = file.reader(&mut phase)?;
            let length = view.length();
            let mut copied = [0; 17];
            view.read_exact(0, &mut copied)?;
            let range = view.read_exact(length, &mut [0; 1]);
            drop(view);
            let retained = file.fields()?.handle.raw() == raw && file.fields()?.handle.is_armed();
            let custody = claim.in_custody();
            let writable = std::fs::OpenOptions::new().write(true).open(&path);
            let sharing_rejected = writable.is_err();
            drop(writable);
            Ok((length, copied, range, retained, custody, sharing_rejected, unopened, unobserved))
        })();
        let result = ClaimError::finish(result, file.close());
        let result = ClaimError::finish(
            result,
            std::fs::remove_file(&path).map_err(|e| ClaimError::new("fixture_remove_file", e)),
        );
        let result = ClaimError::finish(
            result,
            std::fs::remove_dir(directory)
                .map_err(|e| ClaimError::new("fixture_remove_directory", e)),
        );
        let (length, copied, range, retained, custody, sharing_rejected, unopened, unobserved) =
            result?;
        assert_eq!(unopened, Some("native_read_grant"));
        assert_eq!(unobserved, Some("native_read_unobserved"));
        assert_eq!(length, contents.len() as u64);
        assert_eq!(&copied, prefix);
        assert_eq!(range.err().map(|error| error.operation()), Some("diagnostic_read_range"));
        assert!(retained && sharing_rejected);
        assert_eq!(custody, 1);
        assert_eq!(claim.in_custody(), 0);
        Ok(())
    })
}
