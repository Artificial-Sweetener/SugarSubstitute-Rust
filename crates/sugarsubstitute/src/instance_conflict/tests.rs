// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real coordinator and execution owner with a platform-neutral native boundary.
use super::*;
use std::sync::{Condvar, Mutex};

#[derive(Clone, Default)]
struct Boundary(Arc<Mutex<Vec<&'static str>>>);
impl ConflictBackend for Boundary {
    type Error = std::io::Error;
    fn execute(&self, operation: Op, _: &JobContext) -> std::io::Result<Observation> {
        let mut events = self.0.lock().map_err(|_| std::io::Error::other("boundary poisoned"))?;
        Ok(match operation {
            Op::Discover => {
                events.push("discover");
                Observation::Owner("other-installation".into())
            }
            Op::Close => {
                events.push("close");
                Observation::Released
            }
            Op::Force => {
                events.push("force");
                Observation::Released
            }
            Op::Observe => Observation::Waiting,
            Op::Retire => {
                events.push("retire");
                Observation::Retired
            }
        })
    }
    fn relaunch(&self, _: &JobContext) -> std::io::Result<()> {
        let mut events = self.0.lock().map_err(|_| std::io::Error::other("boundary poisoned"))?;
        if events.last() != Some(&"retire") {
            return Err(std::io::Error::other("relaunch before retirement"));
        }
        events.push("relaunch");
        Ok(())
    }
    fn retire_relaunch(&self) -> std::io::Result<()> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("boundary poisoned"))?
            .push("retire-relaunch");
        Ok(())
    }
}
#[derive(Default)]
struct Wake {
    generation: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut generation = self.generation.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("wake poisoned")),
            )
        })?;
        *generation += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn advance(
    service: &mut Service<Boundary>,
    wake: &Wake,
    reached: impl Fn(&Service<Boundary>) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let generation = *wake.generation.lock().map_err(|_| "wake poisoned")?;
        service.drive()?;
        if reached(service) {
            return Ok(());
        }
        let current = wake.generation.lock().map_err(|_| "wake poisoned")?;
        let (observed, timeout) = wake
            .changed
            .wait_timeout_while(
                current,
                deadline.saturating_duration_since(Instant::now()),
                |value| *value == generation,
            )
            .map_err(|_| "wake poisoned")?;
        drop(observed);
        if timeout.timed_out() {
            service.drive()?;
            return if reached(service) {
                Ok(())
            } else {
                Err("conflict completion missing".into())
            };
        }
    }
}
#[test]
fn portable_backend_close_retires_before_relaunch_and_rejoins_execution() -> Result<()> {
    let boundary = Boundary::default();
    let mut service = Service::new(boundary.clone());
    let wake = Arc::new(Wake::default());
    service.initialize(wake.clone())?;
    advance(&mut service, &wake, |service| service.phase == Phase::Ready)?;
    service.choose(ConflictAction::Force);
    assert_eq!(service.phase, Phase::Ready);
    service.choose(ConflictAction::Close);
    advance(&mut service, &wake, Service::finished)?;
    assert!(service.continued());
    assert!(service.owner.is_none());
    assert_eq!(
        *boundary.0.lock().map_err(|_| "boundary poisoned")?,
        ["discover", "close", "retire", "relaunch"]
    );
    Ok(())
}
#[test]
fn cancellation_uses_same_port_without_closing_or_relaunching_other_copy() -> Result<()> {
    let boundary = Boundary::default();
    let mut service = Service::new(boundary.clone());
    let wake = Arc::new(Wake::default());
    service.initialize(wake.clone())?;
    advance(&mut service, &wake, |service| service.phase == Phase::Ready)?;
    service.choose(ConflictAction::Cancel);
    advance(&mut service, &wake, Service::finished)?;
    assert!(!service.continued());
    assert_eq!(*boundary.0.lock().map_err(|_| "boundary poisoned")?, ["discover", "retire"]);
    Ok(())
}
#[test]
fn cancellation_before_initialization_has_no_native_effects() -> Result<()> {
    let boundary = Boundary::default();
    let mut service = Service::new(boundary.clone());
    service.choose(ConflictAction::Cancel);
    assert!(service.finished());
    assert!(!service.continued());
    assert!(boundary.0.lock().map_err(|_| "boundary poisoned")?.is_empty());
    Ok(())
}
