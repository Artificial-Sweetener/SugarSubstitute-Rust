// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose two actual Parents, actual Pending leases, one real output job and independent parsers.
//! These tests authenticate no peer and create no Granted, process or readiness proof.
use super::*;
use crate::production::coordinator_backend::output_fixture as fixture;
use std::{
    error::Error,
    num::{NonZeroU64, NonZeroUsize},
};
use sugarsubstitute_application::transcript::TranscriptStore;
use sugarsubstitute_contracts::transcript::{OutputBatch, OutputStream};
use sugarsubstitute_execution::windows_output::boundary_fixture::OutputFixture;
type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
#[path = "fanout_rig.rs"]
mod rig;
use rig::{Rig, idle, parent, pull, text};

#[test]
fn two_parent_transcripts_receive_same_original_batches_and_independent_ack_ownership() -> TestResult
{
    let rig = Rig::new()?;
    let external = OutputFixture::new();
    external.push(OutputStream::Stdout, b"alpha\n")?;
    let (mut held, leases) = fixture::shared(&rig.client, &rig.native, &external)
        .map_err(|cause| cause as Box<dyn Error>)?;
    let backend = held.backend.as_mut();
    let [a, b, _] = leases;
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 6)
    })?;
    let mut first = parent(a)?;
    let mut second = parent(b)?;
    let mut one = TranscriptStore::new(a.resource);
    let mut two = TranscriptStore::new(b.resource);
    let first_batch = pull(&mut first, a, backend, &mut one)?;
    assert_eq!(first_batch.sequence.get(), 1);
    assert!(!backend.output_loan_available(a).map_err(|cause| cause as Box<dyn Error>)?);
    assert!(backend.output_loan_available(b).map_err(|cause| cause as Box<dyn Error>)?);
    let second_batch = pull(&mut second, b, backend, &mut two)?;
    assert_eq!(first_batch.sequence, second_batch.sequence);
    assert_eq!(text(&one)?, text(&two)?);
    assert_eq!(one.diagnostics().skipped_batches, [0, 0]);
    let receipt = &first.output.as_ref().ok_or("Original Parent delivery absent")?.receipt;
    assert!(backend.acknowledge_output(b, receipt).is_err());
    assert!(first.consume_output_ack(a, NonZeroU64::new(2), backend).is_err());
    assert!(!backend.output_loan_available(a).map_err(|cause| cause as Box<dyn Error>)?);
    first
        .consume_output_ack(a, Some(first_batch.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(second.output.is_some());
    assert!(backend.output_loan_available(a).map_err(|cause| cause as Box<dyn Error>)?);
    idle(&mut first, a, backend)?;
    assert!(second.output.is_some());
    second
        .consume_output_ack(b, Some(second_batch.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(backend.output_loan_available(a).map_err(|cause| cause as Box<dyn Error>)?);
    external.push(OutputStream::Stdout, b"beta\n")?;
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 11)
    })?;
    let next_b = pull(&mut second, b, backend, &mut two)?;
    let next_a = pull(&mut first, a, backend, &mut one)?;
    assert_eq!(next_a.sequence.get(), 2);
    assert_eq!(next_a.sequence, next_b.sequence);
    assert_eq!(text(&one)?, vec!["alpha".to_owned(), "beta".to_owned()]);
    assert_eq!(text(&one)?, text(&two)?);
    second
        .consume_output_ack(b, Some(next_b.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    fixture::disconnect(backend, a).map_err(|cause| cause as Box<dyn Error>)?;
    // A's actual policy disconnect releases only A's unacknowledged matching interest.
    assert!(backend.output_loan_available(b).map_err(|cause| cause as Box<dyn Error>)?);
    external.push(OutputStream::Stdout, b"tail\n")?;
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 16)
    })?;
    let last = pull(&mut second, b, backend, &mut two)?;
    assert_eq!(last.sequence.get(), 3);
    assert!(!backend.output_loan_available(b).map_err(|cause| cause as Box<dyn Error>)?);
    fixture::close(backend, b.resource, &rig.client, &rig.native)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(second.output.is_some());
    assert!(second.consume_output_ack(b, NonZeroU64::new(last.id.get() + 1), backend).is_err());
    second
        .consume_output_ack(b, Some(last.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(second.output.is_none());
    idle(&mut second, b, backend)?;
    rig.until(|| fixture::retire(backend, a.resource).map_err(|cause| cause as Box<dyn Error>))?;
    assert!(external.closes() > 0);
    drop((first, second, held));
    rig.joined();
    Ok(())
}

#[test]
fn late_parent_gets_exact_missing_prefix_and_historical_eof_without_sequence_rewrite() -> TestResult
{
    let rig = Rig::new()?;
    let external = OutputFixture::new();
    external.push(OutputStream::Stdout, b"before\n")?;
    let (mut held, [a, b, c]) = fixture::shared(&rig.client, &rig.native, &external)
        .map_err(|cause| cause as Box<dyn Error>)?;
    let backend = held.backend.as_mut();
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 7)
    })?;
    let mut first = parent(a)?;
    let mut second = parent(b)?;
    let mut one = TranscriptStore::new(a.resource);
    let mut two = TranscriptStore::new(a.resource);
    let initial = pull(&mut first, a, backend, &mut one)?;
    first
        .consume_output_ack(a, Some(initial.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    external.push(OutputStream::Stdout, b"after\n")?;
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 13)
    })?;
    let late = pull(&mut second, b, backend, &mut two)?;
    let current = pull(&mut first, a, backend, &mut one)?;
    assert_eq!(late.sequence.get(), 2);
    assert_eq!(late.gap.map(|gap| (gap.dropped_bytes(), gap.skipped_batches())), Some((7, 1)));
    assert!(current.gap.is_none());
    second
        .consume_output_ack(b, Some(late.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    first
        .consume_output_ack(a, Some(current.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    external.end(OutputStream::Stdout);
    external.end(OutputStream::Stderr);
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource).map_err(|cause| cause as Box<dyn Error>)?.eof
            == [true, true])
    })?;
    for _ in 0..2 {
        let first_end = pull(&mut first, a, backend, &mut one)?;
        let second_end = pull(&mut second, b, backend, &mut two)?;
        assert!(first_end.end && second_end.end);
        assert_eq!(first_end.stream, second_end.stream);
        first
            .consume_output_ack(a, Some(first_end.id), backend)
            .map_err(|cause| cause as Box<dyn Error>)?;
        second
            .consume_output_ack(b, Some(second_end.id), backend)
            .map_err(|cause| cause as Box<dyn Error>)?;
    }
    let mut late_parent = parent(c)?;
    let mut third = TranscriptStore::new(c.resource);
    for _ in 0..2 {
        let end = pull(&mut late_parent, c, backend, &mut third)?;
        assert!(end.end);
        if end.stream == OutputStream::Stdout {
            assert_eq!(end.sequence.get(), 3);
            assert_eq!(
                end.gap.map(|gap| (gap.dropped_bytes(), gap.skipped_batches())),
                Some((13, 2))
            );
        } else {
            assert_eq!(end.sequence.get(), 1);
            assert!(end.gap.is_none());
        }
        late_parent
            .consume_output_ack(c, Some(end.id), backend)
            .map_err(|cause| cause as Box<dyn Error>)?;
    }
    assert_eq!(third.diagnostics().dropped_bytes, [13, 0]);
    rig.until(|| fixture::retire(backend, a.resource).map_err(|cause| cause as Box<dyn Error>))?;
    drop((first, second, late_parent, held));
    rig.joined();
    Ok(())
}

#[test]
fn native_overflow_is_the_same_exact_gap_for_both_live_parent_transcripts() -> TestResult {
    let rig = Rig::new()?;
    let external = OutputFixture::new();
    external.push(OutputStream::Stdout, b"x")?;
    let (mut held, [a, b, _]) = fixture::shared(&rig.client, &rig.native, &external)
        .map_err(|cause| cause as Box<dyn Error>)?;
    let backend = held.backend.as_mut();
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 1)
    })?;
    let mut first = parent(a)?;
    let mut second = parent(b)?;
    let mut one = TranscriptStore::new(a.resource);
    let mut two = TranscriptStore::new(b.resource);
    let first_batch = pull(&mut first, a, backend, &mut one)?;
    let second_batch = pull(&mut second, b, backend, &mut two)?;
    let bytes = vec![b'z'; 32768];
    for _ in 0..40 {
        external.push(OutputStream::Stdout, &bytes)?;
    }
    rig.until(|| {
        Ok(fixture::snapshot(backend, a.resource)
            .map_err(|cause| cause as Box<dyn Error>)?
            .counters[0]
            .read_bytes
            == 1 + 40 * 32768)
    })?;
    let original =
        fixture::snapshot(backend, a.resource).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(original.counters[0].dropped_bytes > 0);
    first
        .consume_output_ack(a, Some(first_batch.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(backend.output_loan_available(a).map_err(|cause| cause as Box<dyn Error>)?);
    second
        .consume_output_ack(b, Some(second_batch.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    let next_a = pull(&mut first, a, backend, &mut one)?;
    let next_b = pull(&mut second, b, backend, &mut two)?;
    assert_eq!(next_a.sequence, next_b.sequence);
    assert_eq!(next_a.gap, next_b.gap);
    assert_eq!(next_a.gap.map(|gap| gap.dropped_bytes()), Some(original.counters[0].dropped_bytes));
    assert_eq!(
        next_a.gap.map(|gap| gap.skipped_batches()),
        Some(original.counters[0].dropped_batches)
    );
    assert_eq!(one.diagnostics().dropped_bytes, two.diagnostics().dropped_bytes);
    first
        .consume_output_ack(a, Some(next_a.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    second
        .consume_output_ack(b, Some(next_b.id), backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    rig.until(|| fixture::retire(backend, a.resource).map_err(|cause| cause as Box<dyn Error>))?;
    drop((first, second, held));
    rig.joined();
    Ok(())
}
