// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose admitted role tests over actual shared fixed-record primitives.
use super::*;
use crate::coordinator_entry::fixture::Harness;
/// Execute an owning role's assertions and checked external custody settlement.
pub(crate) fn with_record<const N: usize>(
    original: Option<Vec<u8>>,
    body: impl FnOnce(&mut HeldFile<N>, &mut Fake, &JobContext) -> Result<(), ClaimError>
    + Send
    + 'static,
) -> TestResult {
    let mut harness = Harness::new()?;
    let fake = Fake::with_bytes(original.as_deref());
    let external = fake.clone();
    let spec = harness.spec(Moment::from_nanos(15_000_000_000))?;
    let mut job = harness.client.submit_blocking(
        &harness.scope,
        spec,
        move |context| -> Result<(), ClaimError> {
            let mut fake = fake;
            let memory = Arc::new(
                context
                    .memory()
                    .claim(1024)
                    .map_err(|cause| ClaimError::new("role_fixture_memory", cause))?,
            );
            let mut retained = None;
            let inode = HeldFile::<N>::adopt_fixture_into(
                &mut retained,
                path(),
                &resolved().account_sid,
                memory,
                &mut fake,
                &context,
            )?;
            let file =
                retained.as_mut().ok_or_else(|| ClaimError::invalid("role_fixture_owner"))?;
            body(file, &mut fake, &context)?;
            assert!(lock(&file.stage).is_none());
            assert!(lock(&file.publication).is_none());
            file.retire_fixture(inode, &mut fake, &context)?;
            assert_eq!(fake.live(), 0);
            Ok(())
        },
    )?;
    let done = harness.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    assert_eq!(external.live(), 0);
    harness.close(Moment::from_nanos(15_000_000_000))?;
    harness.join_execution_after_retirement()?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
