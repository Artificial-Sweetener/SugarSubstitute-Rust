// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Selected streams enforce bounds and controls while retaining their original native charge.
use super::*;
use std::io::{Seek, SeekFrom};

#[test]
fn selected_stream_seeks_without_an_image_sized_allocation_and_releases_native_charge() -> TestResult
{
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("source");
        std::fs::write(&path, b"0123456789abcdef")?;
        with_artifact_input(&path, 16, context, |input| {
            assert_eq!(input.len(), 16);
            assert!(!input.is_empty());
            let remaining = context
                .memory()
                .claim(context.memory().bytes() - 64 * 1024)
                .map_err(io::Error::other)?;
            assert_eq!(input.seek(SeekFrom::End(-4))?, 12);
            let mut bytes = [0; 4];
            input.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"cdef");
            assert_eq!(input.read(&mut bytes)?, 0);
            assert!(input.seek(SeekFrom::Current(1)).is_err());
            assert!(input.seek(SeekFrom::Start(17)).is_err());
            assert_eq!(input.stream_position()?, 16);
            input.rewind()?;
            assert!(input.seek(SeekFrom::Current(-1)).is_err());
            input.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"0123");
            drop(remaining);
            Ok::<(), io::Error>(())
        })?;
        let all = context.memory().claim(context.memory().bytes())?;
        drop(all);
        Ok(())
    })
}

#[test]
fn oversized_and_nonregular_sources_never_reach_the_selected_stream_consumer() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("source");
        std::fs::write(&path, b"data")?;
        let mut called = false;
        let rejected = with_artifact_input(&path, 3, context, |_| {
            called = true;
            Ok::<(), io::Error>(())
        });
        assert!(matches!(rejected, Err(ArtifactError::Storage(_))));
        assert!(!called);
        assert!(
            with_artifact_input(root.path(), u64::MAX, context, |_| {
                called = true;
                Ok::<(), io::Error>(())
            })
            .is_err()
        );
        assert!(!called);
        let all = context.memory().claim(context.memory().bytes())?;
        drop(all);
        Ok(())
    })
}

#[test]
fn selected_stream_checks_expiry_after_callback_and_preserves_its_original_error() -> TestResult {
    run(|context, clock| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("source");
        std::fs::write(&path, b"data")?;
        let result = with_artifact_input(&path, 4, context, |input| {
            clock.expire();
            let mut bytes = [0; 1];
            assert!(input.read(&mut bytes).is_err());
            assert!(input.rewind().is_err());
            Err::<(), _>(io::Error::new(io::ErrorKind::InvalidData, "source decoder failure"))
        });
        match result {
            Err(ArtifactError::Retirement { producer, .. }) => {
                assert_eq!(producer.kind(), io::ErrorKind::InvalidData);
                assert_eq!(producer.to_string(), "source decoder failure");
            }
            other => {
                return Err(
                    format!("Expected preserved producer/control failure: {other:?}").into()
                );
            }
        }
        let all = context.memory().claim(context.memory().bytes())?;
        drop(all);
        std::fs::write(path, b"still authored")?;
        Ok(())
    })
}

#[test]
fn selected_stream_never_publishes_a_success_after_original_deadline_expiry() -> TestResult {
    run(|context, clock| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("source");
        std::fs::write(&path, b"")?;
        let result = with_artifact_input(&path, 0, context, |input| {
            assert!(input.is_empty());
            clock.expire();
            Ok::<_, io::Error>(42)
        });
        assert!(matches!(result, Err(ArtifactError::Storage(_))));
        Ok(())
    })
}
