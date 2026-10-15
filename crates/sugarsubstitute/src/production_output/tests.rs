// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize bounded parsing and real source/job charge lifetime without fabricating frames.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::transcript::*;
use sugarsubstitute_supervision::lifecycle_protocol::OutputDelivery;
mod backend_recovery;
mod retirement;
mod support;
use support::*;
type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
#[test]
fn one_pull_retains_original_header_deadline_and_never_acknowledges_shape() -> TestResult {
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    let binding = binding()?;
    let h = header(binding, 2)?;
    let message = owner.prepare_pull(h, Moment::from_nanos(100))?;
    assert!(matches!(message.payload, WirePayload::OutputPull { acknowledged_delivery: None, .. }));
    assert!(owner.prepare_pull(header(binding, 3)?, Moment::from_nanos(101)).is_err());
    let (same, cutoff) = owner.pending_message().ok_or("Pending missing")?;
    assert_eq!(same.header, h);
    assert_eq!(cutoff, Moment::from_nanos(100));
    owner.admitted_request(h)?;
    assert!(owner.pending_message().is_none());
    assert!(owner.admitted_request(h).is_err());
    owner.close();
    assert!(!owner.worker_pending());
    drop(owner);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn foreign_header_rejects_without_consuming_original_pull_slot() -> TestResult {
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    let b = binding()?;
    let mut foreign = header(b, 2)?;
    foreign.connection = Some(ConnectionId::new(2)?);
    assert!(matches!(
        owner.prepare_pull(foreign, Moment::from_nanos(100)),
        Err(OutputError::Identity)
    ));
    assert!(owner.prepare_pull(header(b, 2)?, Moment::from_nanos(100)).is_ok());
    Ok(())
}
#[test]
fn styled_snapshot_keeps_original_source_reservation_after_owner_and_workers_join() -> TestResult {
    let rig = Rig::new()?;
    let owner = output(&rig)?;
    let b = binding()?;
    let source = owner.source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| {
            let _claim = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            let mut state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            worker::apply_delivery(
                &mut state.transcript,
                b,
                OutputDelivery {
                    lease: b.lease,
                    delivery_id: std::num::NonZeroU64::new(1).ok_or(OutputError::Identity)?,
                    stream: OutputStream::Stdout,
                    sequence: StreamSequence::new(1)?,
                    gap: None,
                    end: false,
                    bytes: b"\x1b[31mred\x1b[0m\n",
                },
                &source.custody,
            )
            .map(|p| p.snapshot)
        },
    )?;
    let view = rig.result(&mut job)?.ok_or("View missing")?;
    drop(job);
    drop(owner);
    drop(rig.owner);
    assert_eq!(view.source_bytes(), SOURCE_BYTES);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    let record = view.transcript().records().next().ok_or("Record absent")?;
    assert_eq!(record.text(), "red");
    assert!(!record.runs().is_empty());
    drop(view);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_source_snapshot_capacity_rejects_fifth_and_returns_after_drop() -> TestResult {
    let rig = Rig::new()?;
    let owner = output(&rig)?;
    let source = owner.source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| -> Result<(), OutputError> {
            let _claim = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            let state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            let mut views = Vec::with_capacity(4);
            for _ in 0..4 {
                views.push(OutputSnapshot::new(
                    state.transcript.snapshot_tail(128)?,
                    source.custody.clone(),
                ))
            }
            assert!(matches!(
                state.transcript.snapshot_tail(128),
                Err(TranscriptError::SnapshotCapacity)
            ));
            views.pop();
            assert!(state.transcript.snapshot_tail(128).is_ok());
            Ok(())
        },
    )?;
    rig.result(&mut job)?;
    Ok(())
}

#[test]
fn actual_foreign_scope_cannot_parse_under_original_source_admission() -> TestResult {
    let rig = Rig::new()?;
    let owner = output(&rig)?;
    let source = owner.source.clone();
    let foreign =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut job = rig.client.submit_blocking(
        &foreign,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| -> Result<bool, OutputError> {
            Ok(matches!(source.check(&context), Err(OutputError::Identity)))
        },
    )?;
    assert!(rig.result(&mut job)?);
    drop(job);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    drop(owner);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(foreign.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn shared_render_source_keeps_one_actual_pin_and_charge_until_final_clone() -> TestResult {
    let rig = Rig::new()?;
    let owner = output(&rig)?;
    let source = owner.source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| -> Result<OutputSnapshot, OutputError> {
            let _scratch = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            let state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            let original =
                OutputSnapshot::new(state.transcript.snapshot_tail(128)?, source.custody.clone());
            let clones = [original.clone(), original.clone(), original.clone(), original.clone()];
            let other = [
                state.transcript.snapshot_tail(1)?,
                state.transcript.snapshot_tail(1)?,
                state.transcript.snapshot_tail(1)?,
            ];
            assert!(matches!(
                state.transcript.snapshot_tail(1),
                Err(TranscriptError::SnapshotCapacity)
            ));
            drop(other);
            drop(clones);
            Ok(original)
        },
    )?;
    let view = rig.result(&mut job)?;
    let last = view.clone();
    let revision = view.transcript().revision();
    drop((job, view, owner));
    drop(rig.owner);
    assert_eq!(last.transcript().revision(), revision);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    drop(last);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn empty_frame_submission_preserves_original_pull_before_any_factory() -> TestResult {
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    let header = header(binding()?, 2)?;
    owner.prepare_pull(header, Moment::from_nanos(100))?;
    owner.admitted_request(header)?;
    let mut frame = None;
    assert!(matches!(
        owner.submit_frame(&rig.client, &rig.scope, JobId::new(2)?, &mut frame),
        Err(OutputError::Order)
    ));
    assert_eq!(owner.pending.ok_or("Original pull missing")?.header, header);
    assert!(!owner.worker_pending());
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    Ok(())
}

#[test]
fn real_cpu_raster_returns_complete_guarded_output_source_until_result_disposal() -> TestResult {
    use sugarsubstitute_contracts::presentation::{FrameIdentity, FrameShape};
    use sugarsubstitute_splash::{
        AnimationFrame, Appearance, ProgressFrame, SPLASH_CACHE_RESERVATION_BYTES,
        SplashRenderRequest, SplashRenderer,
    };
    let rig = Rig::new()?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut construct = OutputOwner::submit_new(
        &rig.client,
        &rig.scope,
        jobs.allocate()?,
        binding()?,
        Moment::from_nanos(100),
    )?;
    let owner = rig.result(&mut construct)?;
    let source = owner.source.clone();
    let mut publish = rig.client.submit_blocking(
        &rig.scope,
        spec(jobs.allocate()?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| -> Result<OutputSnapshot, OutputError> {
            source.check(&context)?;
            let state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            Ok(OutputSnapshot::new(state.transcript.snapshot_tail(1)?, source.custody.clone()))
        },
    )?;
    let view = rig.result(&mut publish)?;
    let render_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cpu_spec = |id: JobId, bytes| -> TestResult<JobSpec> {
        Ok(JobSpec {
            id,
            operation: Operation::FramePrepare,
            class: JobClass::CpuInteractive,
            deadline: None,
            reservation_bytes: bytes,
            replacement: Some(ReplacementKey {
                surface: SurfaceId::new(1)?,
                revision: Generation::new(id.get())?,
            }),
        })
    };
    let mut initialize = rig.client.submit_cpu(
        &render_scope,
        cpu_spec(jobs.allocate()?, SPLASH_CACHE_RESERVATION_BYTES)?,
        |context| SplashRenderer::initialize("en".to_owned(), context.memory().clone()),
    )?;
    let renderer = rig.result(&mut initialize)??;
    let request = SplashRenderRequest {
        shape: FrameShape::new(558, 558, 1.0)?,
        identity: FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        appearance: Appearance::demo(),
        character: AnimationFrame::bootstrap(),
        feedback: ProgressFrame { completed: 0.0, activity: None },
        bootstrap: true,
        console: Some(crate::preparation_task::RenderConsole::from(view.clone())),
    };
    let bytes = request.reservation_bytes();
    let mut raster = rig.client.submit_cpu(
        &render_scope,
        cpu_spec(jobs.allocate()?, bytes)?,
        move |context| renderer.prepare(request, context.memory().clone()),
    )?;
    let result = rig.result(&mut raster)?;
    assert!(result.frame.is_ok());
    assert_eq!(
        result.console.as_ref().ok_or("Original render source missing")?.resource(),
        view.transcript().resource()
    );
    drop((construct, publish, initialize, raster, owner, view));
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    drop(result);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(render_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn finite_domain_ingestion_preserves_independent_stream_gap_utf8_and_sequence() -> TestResult {
    let rig = Rig::new()?;
    let owner = output(&rig)?;
    let b = binding()?;
    let source = owner.source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| -> Result<OutputSnapshot, OutputError> {
            source.check(&context)?;
            let _scratch = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            let mut state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            let deliveries = [
                (OutputStream::Stdout, 1, None, false, b"old\rnew\n\xe2\x82".as_slice()),
                (
                    OutputStream::Stdout,
                    3,
                    Some(OutputGap::new(4, 1)?),
                    false,
                    b"\x1b[32mgreen\x1b[0m\n".as_slice(),
                ),
                (OutputStream::Stderr, 1, None, true, b"bad\xff\n".as_slice()),
            ];
            for (index, (stream, sequence, gap, end, bytes)) in deliveries.into_iter().enumerate() {
                let parsed = worker::apply_delivery(
                    &mut state.transcript,
                    b,
                    OutputDelivery {
                        lease: b.lease,
                        delivery_id: NonZeroU64::new(index as u64 + 1)
                            .ok_or(OutputError::Identity)?,
                        stream,
                        sequence: StreamSequence::new(sequence)?,
                        gap,
                        end,
                        bytes,
                    },
                    &source.custody,
                )?;
                assert!(parsed.change.is_some());
                drop(parsed);
            }
            let before = state.transcript.revision();
            let duplicate = worker::apply_delivery(
                &mut state.transcript,
                b,
                OutputDelivery {
                    lease: b.lease,
                    delivery_id: NonZeroU64::new(4).ok_or(OutputError::Identity)?,
                    stream: OutputStream::Stdout,
                    sequence: StreamSequence::new(3)?,
                    gap: None,
                    end: false,
                    bytes: b"duplicate",
                },
                &source.custody,
            );
            assert!(matches!(duplicate, Err(OutputError::Transcript(TranscriptError::Sequence))));
            assert_eq!(state.transcript.revision(), before);
            Ok(OutputSnapshot::new(state.transcript.snapshot_tail(128)?, source.custody.clone()))
        },
    )?;
    let view = rig.result(&mut job)?;
    let diagnostics = view.transcript().diagnostics();
    assert_eq!(diagnostics.dropped_bytes, [4, 0]);
    assert_eq!(diagnostics.skipped_batches, [1, 0]);
    assert_eq!(diagnostics.unicode_replacements, 2);
    assert!(view.transcript().records().any(|r| r.text() == "green"));
    assert!(view.transcript().records().any(|r| r.text() == "bad\u{fffd}"));
    Ok(())
}

mod pregrant;
