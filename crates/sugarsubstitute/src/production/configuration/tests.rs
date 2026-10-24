// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual preparation/opaque failure custody without reading process environment.
#[path = "tests/input.rs"]
mod input;
use super::*;
use std::{
    ffi::OsString,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobCompletion, JobOutcome, Moment,
    ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{EntryPreparationCause, ExecutionOwner};
type TestResult = Result<(), Box<dyn Error>>;
/// Inject epoch zero without depending on workstation timing.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Notifications preserve causal progress; elapsed time bounds test failure only.
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Observer poisoned")),
            )
        })?;
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
        let until = Instant::now().checked_add(Duration::from_secs(10)).ok_or("Test deadline")?;
        let mut revision = self.revision.lock().map_err(|_| "Observer poisoned")?;
        loop {
            if let Some(result) = job.try_take_completion() {
                return Ok(result);
            }
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Missing completion")?;
            let (next, timed) =
                self.changed.wait_timeout(revision, remaining).map_err(|_| "Observer poisoned")?;
            revision = next;
            if timed.timed_out() {
                return job.try_take_completion().ok_or_else(|| "Missing completion".into());
            }
        }
    }
}

fn options() -> Result<StartupPaths, Box<dyn Error>> {
    Ok(StartupPaths::parse(
        [
            "--startup-config",
            r"E:\inert\absent.toml",
            "--frontend-package",
            r"E:\inert\frontend",
            "--frontend-metadata",
            r"E:\inert\METADATA",
            "--sugarcubes-root",
            r"E:\inert\cubes",
            "--backend-script",
            r"E:\inert\main.py",
            "--profile-directory",
            r"E:\inert\profile",
        ]
        .map(OsString::from),
    )?)
}

#[test]
fn failed_primary_is_adopted_before_post_provider_runs() -> TestResult {
    struct Primary(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Primary {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Release)
        }
    }
    let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let outcome: Result<(), _> = Err(Primary(dropped.clone()));
    let result = observe_parse(outcome, || {
        assert!(!dropped.load(std::sync::atomic::Ordering::Acquire));
        Err("post")
    });
    assert!(matches!(result, Err(ParseFailure::Primary { after: Some("post"), .. })));
    drop(result);
    assert!(dropped.load(std::sync::atomic::Ordering::Acquire));
    Ok(())
}
#[test]
fn real_schema_failure_preserves_redaction_and_original_whole_claim() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let directory = temporary_directory("failure")?;
    let result = (|| {
        let mut paths = options()?.into_paths();
        paths[0] = directory.join("original.toml");
        let input = StartupPaths::new(paths)?;
        std::fs::write(input.configuration(), b"secret-token = broken")?;
        let mut job = submit(
            &client,
            &scope,
            &JobSequence::new(),
            input.into(),
            EntrySetupBudget::new(Moment::from_nanos(0))?,
        )?;
        let completion = wake.completion(&mut job)?;
        let memory = completion.memory.clone();
        let JobOutcome::Failure(failure) = completion.outcome else {
            return Err("Expected schema failure".into());
        };
        let EntryPreparationCause::Factory(ConfigurationCause::Schema { source, after: None }) =
            failure.cause()
        else {
            return Err("Schema cause lost".into());
        };
        assert!(source.source().is_some());
        if let EntryPreparationCause::Factory(cause) = failure.cause() {
            assert!(cause.post_control().is_none());
        }
        assert!(failure.source().is_some());
        assert!(!format!("{failure:?}").contains("secret"));
        assert!(memory.claim(1).is_err());
        drop(failure);
        assert_eq!(
            memory.claim(CONFIGURATION_RESERVATION_BYTES)?.bytes(),
            CONFIGURATION_RESERVATION_BYTES
        );
        Ok(())
    })();
    std::fs::remove_file(directory.join("original.toml"))?;
    std::fs::remove_dir(directory)?;
    result
}
#[test]
fn real_post_cancel_cause_stays_visible_without_replacing_read_or_schema_source() -> TestResult {
    for schema in [false, true] {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
        let client = owner.client();
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let original = scope.clone();
        let retained = Arc::new(Mutex::new(None));
        let destination = retained.clone();
        let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
        let spec = JobSpec {
            id: JobSequence::new().allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(setup.until()),
            reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
            replacement: None,
        };
        let mut job = client
            .submit_entry_preparation(&scope, spec, setup, move |context| {
                let parser = parse_startup_configuration(b"secret-token = broken");
                original.cancel();
                let after = match context.observe_control() {
                    Err(cause) => cause,
                    Ok(_) => return Err(ConfigurationCause::TooLarge),
                };
                let cause = if schema {
                    let source = match parser {
                        Err(cause) => cause,
                        Ok(_) => return Err(ConfigurationCause::TooLarge),
                    };
                    ConfigurationCause::Schema { source: Box::new(source), after: Some(after) }
                } else {
                    ConfigurationCause::Read {
                        source: std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "secret-read",
                        ),
                        after: Some(after),
                    }
                };
                *destination.lock().map_err(|_| ConfigurationCause::TooLarge)? = Some(cause);
                Ok::<_, ConfigurationCause>(())
            })
            .map_err(|rejected| rejected.error)?;
        let completion = wake.completion(&mut job)?;
        let memory = completion.memory.clone();
        drop(completion);
        let cause =
            retained.lock().map_err(|_| "Retained cause poisoned")?.take().ok_or("Cause lost")?;
        assert!(cause.post_control().is_some());
        let first = cause.source().ok_or("Primary source lost")?;
        if schema {
            assert!(first.downcast_ref::<StartupConfigurationError>().is_some());
        } else {
            assert_eq!(
                first.downcast_ref::<std::io::Error>().ok_or("Read source lost")?.kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
        let diagnostic = format!("{cause:?}");
        assert!(diagnostic.contains("post_control: Some(EntryPreparationError"));
        assert!(!diagnostic.contains("secret"));
        assert!(memory.claim(1).is_err());
        drop(cause);
        assert_eq!(
            memory.claim(CONFIGURATION_RESERVATION_BYTES)?.bytes(),
            CONFIGURATION_RESERVATION_BYTES
        );
    }
    Ok(())
}
#[test]
fn real_parser_success_is_borrowed_under_same_preparation_claim_without_native_get() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let directory = temporary_directory("success")?;
    let result = (|| {
        let mut paths = options()?.into_paths();
        paths[0] = directory.join("original.toml");
        let input = StartupPaths::new(paths)?;
        std::fs::write(
            input.configuration(),
            br#"schema_version=1
mode="attach"
port=8188
profile="original_profile"
locale="en"
theme="dark"
reduced_motion=false
[attach]
origin="http://localhost:8188"
"#,
        )?;
        let expected = input.configuration().to_path_buf();
        let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
        let spec = JobSpec {
            id: JobSequence::new().allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(setup.until()),
            reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
            replacement: None,
        };
        let mut job = client
            .submit_entry_preparation(&scope, spec, setup, move |context| {
                load_configuration(&context, input)
            })
            .map_err(|rejected| rejected.error)?;
        let completion = wake.completion(&mut job)?;
        let memory = completion.memory.clone();
        let JobOutcome::Success(prepared) = completion.outcome else {
            return Err("Expected parser success".into());
        };
        assert_eq!(prepared.value().options.configuration(), expected);
        assert!(matches!(
            prepared.value().configuration.connection(),
            sugarsubstitute_contracts::configuration::ConnectionConfiguration::Attach(_)
        ));
        assert!(memory.claim(1).is_err());
        drop(prepared);
        assert_eq!(
            memory.claim(CONFIGURATION_RESERVATION_BYTES)?.bytes(),
            CONFIGURATION_RESERVATION_BYTES
        );
        Ok(())
    })();
    std::fs::remove_file(directory.join("original.toml"))?;
    std::fs::remove_dir(directory)?;
    result
}
fn temporary_directory(kind: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let directory = std::env::temp_dir().join(format!(
        "sugarsubstitute-config-{kind}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    Ok(directory)
}
