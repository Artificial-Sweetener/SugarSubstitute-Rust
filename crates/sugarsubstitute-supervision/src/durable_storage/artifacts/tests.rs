// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise selected artifacts with real execution, files, bounded allocation and original expiry.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
mod input;
#[cfg(windows)]
mod retained;
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
#[derive(Default)]
pub(super) struct TestClock(AtomicU64);
impl TestClock {
    fn expire(&self) {
        self.0.store(2_000_000_000, Ordering::SeqCst);
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut count = self.0.lock().map_err(|cause| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other(cause.to_string())),
            )
        })?;
        *count += 1;
        self.1.notify_all();
        Ok(())
    }
}
pub(super) fn run(
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1_000_000_000)),
            reservation_bytes: 2 * 1024 * 1024,
            replacement: None,
        },
        move |context| {
            let result = body(&context, &clock);
            *worker_observed
                .lock()
                .map_err(|_| io::Error::other("Artifact observer poisoned"))? = Some(result);
            Ok::<(), io::Error>(())
        },
    )?;
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Artifact wake poisoned")?;
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(completion) = job.try_take_completion() {
            assert!(matches!(completion.outcome, JobOutcome::Success(()) | JobOutcome::TimedOut));
            break;
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Artifact observer expired")?;
        let (_guard, timeout) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Artifact wake poisoned")?,
                remaining,
                |count| *count == revision,
            )
            .map_err(|_| "Artifact wake poisoned")?;
        if timeout.timed_out() {
            return Err("Artifact worker did not finish".into());
        }
    }
    drop(job);
    drop(executor);
    observed
        .lock()
        .map_err(|_| "Artifact observer poisoned")?
        .take()
        .ok_or("Artifact assertions did not complete")?
}

/// Replacing the selected directory entry must preserve other links to its previous bytes.
#[test]
fn artifact_replacement_never_truncates_another_link_to_the_previous_output() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("selected");
        let other = root.path().join("authored");
        std::fs::write(&path, b"preserved")?;
        std::fs::hard_link(&path, &other)?;
        write_artifact(
            &path,
            false,
            ArtifactPublication::ReplaceSelected,
            32,
            context,
            |output| output.write_all(b"new artifact"),
        )?;
        assert_eq!(std::fs::read(path)?, b"new artifact");
        assert_eq!(std::fs::read(other)?, b"preserved");
        Ok(())
    })
}

/// Create-only publication preserves existing bytes, while explicit replacement changes one leaf.
#[test]
fn selected_artifact_publication_preserves_occupied_outputs_until_explicit_replacement()
-> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("日本語😀.json");
        write_artifact(&path, false, ArtifactPublication::CreateOnly, 32, context, |output| {
            output.write_all(b"original")
        })?;
        assert!(
            write_artifact(&path, false, ArtifactPublication::CreateOnly, 32, context, |output| {
                output.write_all(b"collision")
            })
            .is_err()
        );
        assert_eq!(std::fs::read(&path)?, b"original");
        write_artifact(
            &path,
            false,
            ArtifactPublication::ReplaceSelected,
            32,
            context,
            |output| output.write_all(b"replacement"),
        )?;
        assert_eq!(std::fs::read(path)?, b"replacement");
        Ok(())
    })
}

/// Size and producer errors leave the previous artifact authoritative and never activate a prefix.
#[test]
fn artifact_bounds_and_failed_producers_cannot_truncate_selected_outputs() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("receipt");
        std::fs::write(&path, b"preserved")?;
        let result = write_artifact(
            &path,
            false,
            ArtifactPublication::ReplaceSelected,
            4,
            context,
            |output| {
                output.write_all(b"ok")?;
                output.write_all(b"excess")
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path)?, b"preserved");
        let result = write_artifact(
            &path,
            false,
            ArtifactPublication::ReplaceSelected,
            32,
            context,
            |output| {
                output.write_all(b"incomplete")?;
                Err(io::Error::other("feature encoder failed"))
            },
        );
        assert!(matches!(result, Err(ArtifactError::Producer(_))));
        assert_eq!(std::fs::read(path)?, b"preserved");
        Ok(())
    })
}

/// Expiry after a real staged write prevents activation without treating a prefix as completion.
#[test]
fn artifact_expiry_after_native_writes_preserves_the_original_target() -> TestResult {
    run(|context, clock| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("receipt");
        std::fs::write(&path, b"preserved")?;
        let result = write_artifact(
            &path,
            false,
            ArtifactPublication::ReplaceSelected,
            32,
            context,
            |output| {
                output.write_all(b"pending")?;
                clock.expire();
                Ok::<(), io::Error>(())
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(path)?, b"preserved");
        Ok(())
    })
}

/// Selected input enforces its sentinel and holds its original admission for the returned bytes.
#[test]
fn artifact_input_bounds_and_retained_memory_follow_actual_byte_lifetimes() -> TestResult {
    run(|context, _| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("scenarios");
        std::fs::write(&path, b"data")?;
        assert!(read_artifact(&path, 3, context).is_err());
        let bytes = read_artifact(&path, 4, context)?;
        assert_eq!(bytes.as_slice(), b"data");
        assert!(context.memory().claim(context.memory().bytes()).is_err());
        drop(bytes);
        let full = context.memory().claim(context.memory().bytes())?;
        drop(full);
        Ok(())
    })
}

/// Parent creation is explicit; ended controls cannot create a folder or stage.
#[test]
fn artifact_parent_creation_and_original_controls_are_independent_permissions() -> TestResult {
    run(|context, clock| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("nested/receipt");
        assert!(
            write_artifact(&path, false, ArtifactPublication::CreateOnly, 32, context, |output| {
                output.write_all(b"data")
            })
            .is_err()
        );
        assert!(!root.path().join("nested").try_exists()?);
        write_artifact(&path, true, ArtifactPublication::CreateOnly, 32, context, |output| {
            output.write_all(b"data")
        })?;
        clock.expire();
        let blocked = root.path().join("blocked/receipt");
        assert!(
            write_artifact(
                &blocked,
                true,
                ArtifactPublication::CreateOnly,
                32,
                context,
                |output| output.write_all(b"data")
            )
            .is_err()
        );
        assert!(!root.path().join("blocked").try_exists()?);
        Ok(())
    })
}
