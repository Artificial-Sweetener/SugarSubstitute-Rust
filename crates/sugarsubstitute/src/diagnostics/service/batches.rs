// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish one original batch through source-preserving attempts that only the shutdown cutoff ends.
use super::*;
use crate::diagnostics::queue::Batch;

/// Return the original batch and charge when native writer admission rejects it.
struct AttemptFailure {
    batch: Batch,
    cause: io::Error,
}
/// Native writer admission is retried until the shutdown cutoff; the original busy cause survives it.
#[derive(Debug)]
struct ContentionDeadline(io::Error);
impl std::fmt::Display for ContentionDeadline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Diagnostic writer contention outlasted the shutdown cutoff")
    }
}
impl std::error::Error for ContentionDeadline {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
/// Preserve the originating writer rejection alongside a later execution admission failure.
#[derive(Debug)]
struct AdmissionFailure {
    cause: sugarsubstitute_execution::OwnedInputError,
    _contention: Option<io::Error>,
}
impl std::fmt::Display for AdmissionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Diagnostic storage admission failed")
    }
}
impl std::error::Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
/// Keep drained frames and their original charge through rejection, native return and retry.
pub(super) async fn publish(
    batch: Batch,
    client: &ExecutionClient,
    scope: &Scope,
    sink: &Arc<dyn Sink>,
    health: &DiagnosticHealth,
    context: &JobContext,
    sequence: &mut u64,
) -> io::Result<()> {
    // Another process of this installation may hold the shared history writer for as long as
    // its own slow write takes, so a busy writer delays the batch until it frees. Only the
    // shutdown cutoff, once the service stops, ends the wait.
    let cutoff = batch.cutoff.clone();
    let effective_deadline = || {
        sugarsubstitute_contracts::execution::Moment::from_nanos(
            cutoff.load(std::sync::atomic::Ordering::Acquire),
        )
    };
    // Protected storage demands a finite admitted deadline; before shutdown the attempt carries
    // the open-ended one, so only the shutdown cutoff or cancellation stops it.
    let attempt_deadline = || effective_deadline().min(crate::job_deadline::OPEN_ENDED);
    let mut first_busy = None;
    let mut input = Some(batch);
    'attempt: loop {
        if context.cancellation_requested() || context.now() >= effective_deadline() {
            return Err(match first_busy {
                Some(cause) => io::Error::new(io::ErrorKind::TimedOut, ContentionDeadline(cause)),
                None => {
                    io::Error::new(io::ErrorKind::Interrupted, "Diagnostic publication stopped")
                }
            });
        }
        *sequence = sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Diagnostic job identity exhausted"))?;
        let target = sink.clone();
        let observer = health.clone();
        let mut job = client
            .submit_blocking_owned_input(
                scope,
                spec(
                    *sequence,
                    JobClass::InfrastructureOrdinary,
                    Some(attempt_deadline()),
                    BATCH_MEMORY_BYTES,
                )?,
                &mut input,
                move |batch, context| {
                    let result = tracing::subscriber::with_default(
                        tracing::subscriber::NoSubscriber::default(),
                        || target.append(&batch.frames, batch.omitted, &context),
                    );
                    match result {
                        Ok(()) => {
                            drop(batch);
                            Ok(())
                        }
                        Err(cause) if target.overloaded(&cause) => {
                            Err(AttemptFailure { batch, cause })
                        }
                        Err(cause) => {
                            let kind = cause.kind();
                            observer.retain(cause);
                            Err(AttemptFailure {
                                batch,
                                cause: io::Error::new(kind, ObservedSinkFailure),
                            })
                        }
                    }
                },
            )
            .map_err(|cause| {
                io::Error::other(AdmissionFailure { cause, _contention: first_busy.take() })
            })?;
        loop {
            client.drain_completions(NonZeroUsize::MIN).map_err(io::Error::other)?;
            if let Some(completion) = job.try_take_completion() {
                match completion.outcome {
                    JobOutcome::Success(()) => return Ok(()),
                    JobOutcome::Failure(AttemptFailure { batch, cause }) => {
                        if sink.overloaded(&cause) {
                            if first_busy.is_none() {
                                first_busy = Some(cause);
                            }
                            input = Some(batch);
                            context
                                .sleep_until(
                                    context
                                        .now()
                                        .checked_add(Duration::from_millis(10))
                                        .map_err(io::Error::other)?
                                        .min(effective_deadline()),
                                )
                                .await;
                            continue 'attempt;
                        }
                        return Err(cause);
                    }
                    other => {
                        return Err(first_busy.map_or_else(
                            || {
                                io::Error::other(format!(
                                    "Diagnostic storage terminal: {:?}",
                                    other.kind()
                                ))
                            },
                            |cause| {
                                io::Error::new(io::ErrorKind::TimedOut, ContentionDeadline(cause))
                            },
                        ));
                    }
                }
            }
            if context.cancellation_requested() || context.now() >= effective_deadline() {
                job.cancel();
            }
            context
                .sleep_until(
                    context
                        .now()
                        .checked_add(Duration::from_millis(10))
                        .map_err(io::Error::other)?,
                )
                .await;
        }
    }
}
