// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe filesystem facts under original job controls without granting ownership or custody.
use crate::durable_storage::control::effect;
use std::{
    fs::Metadata,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Inspect the named object without following its final reparse point or symlink.
/// The result is an inert observation. Callers retain ancestry, authorize the name and decide
/// which object kinds are permitted; this operation establishes no protection or deletion right.
pub(crate) fn inspect(path: impl AsRef<Path>, context: &JobContext) -> io::Result<Metadata> {
    effect(context, || std::fs::symlink_metadata(path.as_ref()))
}

/// Resolve an existing name using the platform's canonical spelling and original controls.
/// Resolution follows links as before; callers retain their own pins and security validation.
/// A returned path is an observation and cannot substitute for an identity-bound native handle.
pub(crate) fn canonical_path(path: impl AsRef<Path>, context: &JobContext) -> io::Result<PathBuf> {
    effect(context, || std::fs::canonicalize(path.as_ref()))
}

/// Observe following-link existence, preserving native errors and original control outcomes.
/// Only a native missing-object result becomes false. Permission and control failures stay errors.
pub(crate) fn exists(path: impl AsRef<Path>, context: &JobContext) -> io::Result<bool> {
    effect(context, || path.as_ref().try_exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};

    /// Preserve Unicode names, inert metadata and actual native failure categories.
    #[test]
    fn observations_preserve_native_names_lengths_and_missing_object_results() -> TestResult {
        worker::run_reserved(2 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let original = root.path().join("文档-α.txt");
            std::fs::write(&original, b"preserved")?;
            let metadata = inspect(&original, context)?;
            assert!(metadata.is_file());
            assert_eq!(metadata.len(), 9);
            let resolved = canonical_path(&original, context)?;
            assert_eq!(resolved.file_name(), original.file_name());
            assert_eq!(std::fs::read(resolved)?, b"preserved");
            assert!(exists(&original, context)?);
            let absent = root.path().join("absent");
            assert!(!exists(&absent, context)?);
            assert_eq!(
                inspect(&absent, context).err().ok_or("Missing object inspected")?.kind(),
                io::ErrorKind::NotFound
            );
            assert_eq!(
                canonical_path(&absent, context).err().ok_or("Missing object resolved")?.kind(),
                io::ErrorKind::NotFound
            );
            Ok(())
        })
    }

    /// Original expiry takes precedence over both success and missing-object observations.
    #[test]
    fn original_expiry_rejects_observations_of_present_and_absent_names() -> TestResult {
        worker::run_reserved_expiring(2 * 1024 * 1024, |context, clock| {
            let root = tempfile::tempdir()?;
            let original = root.path().join("original");
            std::fs::write(&original, b"preserved")?;
            clock.expire();
            for path in [original.clone(), root.path().join("absent")] {
                assert_eq!(
                    inspect(&path, context).err().ok_or("Expired metadata observed")?.kind(),
                    io::ErrorKind::TimedOut
                );
                assert_eq!(
                    canonical_path(&path, context).err().ok_or("Expired path resolved")?.kind(),
                    io::ErrorKind::TimedOut
                );
                assert_eq!(
                    exists(&path, context).err().ok_or("Expired presence observed")?.kind(),
                    io::ErrorKind::TimedOut
                );
            }
            assert_eq!(std::fs::read(original)?, b"preserved");
            Ok(())
        })
    }
}
