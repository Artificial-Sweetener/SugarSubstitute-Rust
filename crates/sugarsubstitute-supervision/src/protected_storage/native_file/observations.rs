// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded observed failures without treating repeated aliases as new custody.
use super::*;
use crate::native_claims::files::test_support::{TestResult, admitted};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn empty(context: &JobContext) -> Result<NativeFile, ClaimError> {
    let memory = context
        .memory()
        .claim(1024 * 1024)
        .map_err(|cause| ClaimError::new("fixture_memory", cause))?;
    NativeFile::reserve(
        1,
        1024,
        FileGrant::MetadataReadOnly,
        &mut Some((
            NativeName::Ordinary(std::path::PathBuf::from("fixture-no-native-open")),
            NativeFileAdmission::Ordinary { budget: Budget::Protected(Arc::new(memory)) },
        )),
    )
}

#[test]
fn remembered_alias_does_not_consume_secondary_but_changed_cleanup_does() -> TestResult {
    admitted(|context| {
        let available = match context.memory().claim(context.memory().bytes()) {
            Ok(probe) => probe.bytes(),
            Err(cause) => cause.available_bytes,
        };
        let mut file = empty(&context)?;
        let first = ClaimError::invalid("fixture_first");
        file.remember(first.clone());
        file.remember(first.clone());
        assert!(file.fields()?.secondary_failure.is_none());
        let changed = ClaimError::finish::<()>(
            Err(first.clone()),
            Err(ClaimError::invalid("fixture_cleanup")),
        )
        .err()
        .ok_or_else(|| ClaimError::invalid("fixture_changed_cause"))?;
        assert!(!first.same_observation(&changed));
        file.remember(changed.clone());
        file.remember(changed.clone());
        assert!(
            file.fields()?
                .first_failure
                .as_ref()
                .is_some_and(|cause| cause.same_observation(&first))
        );
        assert!(
            file.fields()?
                .secondary_failure
                .as_ref()
                .is_some_and(|cause| cause.same_observation(&changed))
        );
        drop(file);
        drop(first);
        drop(changed);
        assert_eq!(
            context
                .memory()
                .claim(available)
                .map_err(|cause| ClaimError::new("fixture_credit_return", cause))?
                .bytes(),
            available
        );
        Ok(())
    })
}

#[derive(Debug)]
struct ObservedCauseDrop(Arc<AtomicUsize>);
impl std::fmt::Display for ObservedCauseDrop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("fixture observed external cause")
    }
}
impl std::error::Error for ObservedCauseDrop {}
impl Drop for ObservedCauseDrop {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn unexpected_third_observation_retains_escaped_cause_after_empty_owner_drop() -> TestResult {
    admitted(|context| {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let third = Arc::new(AtomicUsize::new(0));
        let mut file = empty(&context)?;
        file.remember(ClaimError::new("fixture_first", ObservedCauseDrop(first.clone())));
        file.remember(ClaimError::new("fixture_second", ObservedCauseDrop(second.clone())));
        file.remember(ClaimError::new("fixture_third", ObservedCauseDrop(third.clone())));
        assert_eq!(
            file.fields()?.first_failure.as_ref().map(ClaimError::operation),
            Some("fixture_first")
        );
        assert_eq!(
            file.fields()?.secondary_failure.as_ref().map(ClaimError::operation),
            Some("fixture_second")
        );
        drop(file);
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 1);
        assert_eq!(third.load(Ordering::SeqCst), 0);
        // Observes this cause's payload Drop, not allocator free or process RSS.
        Ok(())
    })
}
