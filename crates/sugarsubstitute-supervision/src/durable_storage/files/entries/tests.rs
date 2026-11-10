// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual native enumeration, pinned ancestry and original control outcomes.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::os::windows::{ffi::OsStringExt, fs::OpenOptionsExt};

/// Native names and snapshot metadata survive cursor disposal without granting another open.
#[test]
fn enumeration_retains_native_parent_and_observes_share_exclusive_files() -> TestResult {
    worker::run_reserved(1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let owned = root.path().join("owned");
        fs::create_dir(&owned)?;
        let name = OsString::from_wide(&[b'x' as u16, 0xd800]);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(owned.join(&name))?;
        use std::io::Write;
        file.write_all(b"prefix")?;
        file.sync_all()?;
        let parent = Directory::open(&owned, false)?;
        let mut entries = parent.entries(context)?;
        let entry = entries.next().ok_or("Native entry absent")??;
        assert!(entries.next().is_none());
        drop(entries);
        assert_eq!(entry.file_name(), name);
        assert_eq!(entry.path(), owned.join(&name));
        assert_eq!(entry.metadata()?.len(), 6);
        assert!(entry.file_type()?.is_file());
        assert!(fs::File::open(entry.path()).is_err());
        assert!(
            crate::durable_storage::staging::publish(&owned, &root.path().join("moved")).is_err()
        );
        drop(entry);
        drop(parent);
        drop(file);
        assert_eq!(fs::read(owned.join(name))?, b"prefix");
        Ok(())
    })
}

/// Expired control rejects new cursors, advancement and entry I/O while names stay inert.
#[test]
fn enumeration_reports_expired_creation_advancement_and_entry_effects() -> TestResult {
    worker::run_reserved_expiring(1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("kept"), b"preserved")?;
        let parent = Directory::open(root.path(), false)?;
        let mut entries = parent.entries(context)?;
        let entry = entries.next().ok_or("Native entry absent")??;
        clock.expire();
        assert_eq!(
            parent.entries(context).err().ok_or("Expired cursor opened")?.kind(),
            io::ErrorKind::TimedOut
        );
        for result in [
            entry.metadata().map(|_| ()),
            entry.current_metadata().map(|_| ()),
            entry.file_type().map(|_| ()),
        ] {
            assert_eq!(
                result.err().ok_or("Expired entry accessed")?.kind(),
                io::ErrorKind::TimedOut
            );
        }
        assert_eq!(entry.file_name(), "kept");
        assert_eq!(
            entries
                .next()
                .ok_or("Expiry hidden at EOF")?
                .err()
                .ok_or("Expired entry yielded")?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(entries.next().is_none());
        assert_eq!(fs::read(root.path().join("kept"))?, b"preserved");
        Ok(())
    })
}

/// An empty cursor must report expired original control before reporting native EOF once.
#[test]
fn empty_enumeration_reports_expiry_once_before_terminal_eof() -> TestResult {
    worker::run_reserved_expiring(1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut entries = parent.entries(context)?;
        clock.expire();
        assert_eq!(
            entries
                .next()
                .ok_or("Expiry hidden by empty directory")?
                .err()
                .ok_or("Expired cursor succeeded")?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(entries.next().is_none());
        assert!(entries.next().is_none());
        Ok(())
    })
}

/// The clock fake affects only the boundary; both actual result facts remain available.
#[test]
fn failed_native_effect_preserves_original_cause_when_control_expires() -> TestResult {
    worker::run_reserved_expiring(1024 * 1024, |context, clock| {
        let failure = effect::<()>(context, || {
            clock.expire();
            Err(io::Error::from_raw_os_error(112))
        })
        .err()
        .ok_or("Failed native effect succeeded")?;
        assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
        let details = failure
            .get_ref()
            .and_then(|source| source.downcast_ref::<InterruptedEffect>())
            .ok_or("Original failure lost")?;
        assert_eq!(details.source.raw_os_error(), Some(112));
        assert_eq!(details.control.kind(), io::ErrorKind::TimedOut);
        Ok(())
    })
}

/// Real scope cancellation reaches retained entries and settles only after their native return.
#[test]
fn actual_scope_cancellation_reaches_retained_entries_before_observed_settlement() -> TestResult {
    use crate::setup_storage::test_worker::pipeline::Pipeline;
    use std::sync::{Arc, Condvar, Mutex};
    use sugarsubstitute_contracts::execution::{
        ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation,
    };

    #[derive(Default)]
    struct Checkpoint {
        ready: bool,
        release: bool,
    }
    /// Complete one side of the handshake even when its owner returns early or unwinds, so a
    /// failure on either side ends the test with its cause instead of stranding the other side.
    struct Settle(Arc<(Mutex<Checkpoint>, Condvar)>, fn(&mut Checkpoint));
    impl Drop for Settle {
        fn drop(&mut self) {
            let (lock, wake) = &*self.0;
            (self.1)(&mut lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
            wake.notify_all();
        }
    }
    let mut pipeline = Pipeline::new()?;
    let checkpoint = Arc::new((Mutex::new(Checkpoint::default()), Condvar::new()));
    let _release = Settle(checkpoint.clone(), |state| state.release = true);
    let observed = Arc::new(Mutex::new(None));
    let worker_checkpoint = checkpoint.clone();
    let worker_observed = observed.clone();
    let mut job = pipeline.admit(|client, scope, id| {
        Ok(client.submit_blocking(
            scope,
            JobSpec {
                id,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(2_000_000_000)),
                reservation_bytes: 1024 * 1024,
                replacement: None,
            },
            move |context| {
                let _ready = Settle(worker_checkpoint.clone(), |state| state.ready = true);
                let result: TestResult = (|| {
                    let root = tempfile::tempdir()?;
                    fs::write(root.path().join("kept"), b"preserved")?;
                    let parent = Directory::open(root.path(), false)?;
                    let mut entries = parent.entries(&context)?;
                    let entry = entries.next().ok_or("Native entry absent")??;
                    let (lock, wake) = &*worker_checkpoint;
                    let mut checkpoint = lock.lock().map_err(|_| "Checkpoint poisoned")?;
                    checkpoint.ready = true;
                    wake.notify_all();
                    let checkpoint = wake
                        .wait_while(checkpoint, |state| !state.release)
                        .map_err(|_| "Checkpoint poisoned")?;
                    drop(checkpoint);
                    assert!(context.cancellation_requested());
                    assert_eq!(
                        entry.metadata().err().ok_or("Cancelled entry accessed")?.kind(),
                        io::ErrorKind::Interrupted
                    );
                    assert_eq!(
                        entries
                            .next()
                            .ok_or("Cancellation hidden at EOF")?
                            .err()
                            .ok_or("Cancelled cursor yielded")?
                            .kind(),
                        io::ErrorKind::Interrupted
                    );
                    assert!(entries.next().is_none());
                    assert_eq!(fs::read(root.path().join("kept"))?, b"preserved");
                    Ok(())
                })();
                *worker_observed
                    .lock()
                    .map_err(|_| io::Error::other("Native observation poisoned"))? = Some(result);
                Ok::<_, io::Error>(())
            },
        )?)
    })?;
    let (lock, wake) = &*checkpoint;
    let checkpoint = lock.lock().map_err(|_| "Checkpoint poisoned")?;
    let mut checkpoint =
        wake.wait_while(checkpoint, |state| !state.ready).map_err(|_| "Checkpoint poisoned")?;
    pipeline.admit(|_, scope, _| {
        scope.cancel();
        Ok(())
    })?;
    checkpoint.release = true;
    wake.notify_all();
    drop(checkpoint);
    let completion = pipeline.observe(|| job.try_take_completion())?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    let result = observed
        .lock()
        .map_err(|_| "Native observation poisoned")?
        .take()
        .ok_or("Native cancellation assertions did not complete")?;
    drop(job);
    result
}
