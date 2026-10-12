// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original typed failures while retaining actual native cleanup causes.
use super::types::EntryTransportError;
use crate::native::NativeObservationError;
use std::{error::Error, io};

#[test]
fn cleanup_retains_typed_original_and_each_native_cause_without_identity_flattening() {
    let mut first = EntryTransportError::Malformed;
    first.retain_cleanup(EntryTransportError::native(NativeObservationError::new(
        "first_close",
        io::Error::other("original native close"),
    )));
    first.retain_cleanup(EntryTransportError::Native {
        cause: NativeObservationError::new("identity_restore", io::Error::other("restoration")),
        identity_unrestored: true,
    });
    assert!(first.requires_process_shutdown());
    assert_eq!(first.native_cause().map(NativeObservationError::stage), Some("first_close"));
    assert_eq!(
        first.source().map(ToString::to_string),
        Some(EntryTransportError::Malformed.to_string())
    );
    assert!(
        matches!(&first,EntryTransportError::Retirement {original,..} if matches!(original.as_ref(),EntryTransportError::Malformed))
    );
}

#[path = "admission_tests.rs"]
mod admission;
#[path = "authentication_tests.rs"]
mod authentication;
#[path = "issue_tests.rs"]
mod issue;
#[path = "test_support.rs"]
mod kernel;
#[path = "retirement_tests.rs"]
mod retirement;
#[path = "worker_tests.rs"]
mod worker;
