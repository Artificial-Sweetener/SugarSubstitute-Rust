// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise sole record algorithms through actual election, claims and job admission.
mod backend_release;
pub(crate) mod boundary;
mod clock_failure;
mod contexts;
mod custody;
mod foreign;
mod identity;
mod lifetime;
mod ordinary;
mod publication;
mod read_context;
mod release;
pub(crate) mod role_fixture;
use super::publication as publication_core;
use super::*;
use crate::{
    claims::entry::fixture::{PublicationFixture, with_fresh_publication},
    coordinator_entry::{
        election::{ElectionRecordOrigin, FreshRecordOrigin},
        fixture::resolved,
    },
};
use boundary::{Effect, Fake};
use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{Clock, ExecutionPort, JobContext, JobOutcome, Moment};
type TestResult = Result<(), Box<dyn Error>>;
type Retained = Arc<Mutex<Option<HeldFile<128>>>>;
fn path() -> std::path::PathBuf {
    let namespace = resolved();
    namespace
        .directory
        .join("coordinators")
        .join(hex(namespace.namespace.bytes()))
        .with_extension("record")
}
fn adopt(
    retained: &Retained,
    fake: &mut Fake,
    context: &JobContext,
) -> Result<[u8; 24], ClaimError> {
    let memory = Arc::new(
        context
            .memory()
            .claim(1024)
            .map_err(|cause| ClaimError::new("record_fixture_claim", cause))?,
    );
    HeldFile::adopt_fixture_into(
        &mut lock(retained),
        path(),
        &resolved().account_sid,
        memory,
        fake,
        context,
    )
}
fn finish(fixture: &mut PublicationFixture) -> TestResult {
    let now = fixture.entry.clock.now();
    fixture.entry.close(Moment::from_nanos(now.as_nanos() + 15_000_000_000))?;
    fixture.entry.join_execution_after_retirement()?;
    assert_eq!(fixture.entry.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
