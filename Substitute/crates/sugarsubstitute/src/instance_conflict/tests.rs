// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real coordinator and execution owner with a platform-neutral native boundary.
use super::*;
use crate::{completion_wake::Wake, paused_clock::PausedClock};
use std::sync::Mutex;
use sugarsubstitute_contracts::instance_conflict::ConflictPhase;
use sugarsubstitute_execution::clock_fixture::ManualClock;

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
/// Drive the service on each execution wake until `reached`; real work only makes this slower.
fn advance<B: ConflictBackend>(
    service: &mut Service<B>,
    wake: &Wake,
    reached: impl Fn(&Service<B>) -> bool,
) -> Result<()> {
    wake.until(|| match service.drive() {
        Err(cause) => Some(Err(cause)),
        Ok(()) => reached(service).then_some(Ok(())),
    })
}
#[test]
fn portable_backend_close_retires_before_relaunch_and_rejoins_execution() -> Result<()> {
    let boundary = Boundary::default();
    let mut service = Service::new(boundary.clone());
    service.clock = PausedClock::new();
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
/// A previous copy that keeps running after an ordinary close request until it is ended.
#[derive(Clone, Default)]
struct StalledClose(Boundary);
impl ConflictBackend for StalledClose {
    type Error = std::io::Error;
    fn execute(&self, operation: Op, context: &JobContext) -> std::io::Result<Observation> {
        let observation = self.0.execute(operation, context)?;
        Ok(if operation == Op::Close { Observation::Waiting } else { observation })
    }
    fn relaunch(&self, context: &JobContext) -> std::io::Result<()> {
        self.0.relaunch(context)
    }
    fn retire_relaunch(&self) -> std::io::Result<()> {
        self.0.retire_relaunch()
    }
}
#[test]
fn force_is_offered_after_fifteen_seconds_on_the_execution_clock_alone() -> Result<()> {
    let boundary = StalledClose::default();
    let mut service = Service::new(boundary.clone());
    let clock = ManualClock::new();
    service.clock = clock.clone();
    let wake = Arc::new(Wake::default());
    service.initialize(wake.clone())?;
    advance(&mut service, &wake, |service| service.phase == Phase::Ready)?;
    assert_eq!(service.poll().phase, ConflictPhase::AwaitingDecision);
    service.choose(ConflictAction::Close);
    advance(&mut service, &wake, |service| {
        service.pending.is_none() && matches!(service.phase, Phase::Closing(_))
    })?;
    assert_eq!(service.poll().phase, ConflictPhase::Closing);
    clock.advance(Duration::from_secs(14))?;
    assert_eq!(service.poll().phase, ConflictPhase::Closing);
    service.choose(ConflictAction::Force);
    assert!(matches!(service.phase, Phase::Closing(_)));
    clock.advance(Duration::from_secs(1))?;
    assert_eq!(service.poll().phase, ConflictPhase::ForceOffered);
    service.choose(ConflictAction::Force);
    assert_eq!(service.poll().phase, ConflictPhase::Forcing);
    advance(&mut service, &wake, Service::finished)?;
    assert!(service.continued());
    assert_eq!(
        *boundary.0.0.lock().map_err(|_| "boundary poisoned")?,
        ["discover", "close", "force", "retire", "relaunch"]
    );
    Ok(())
}
#[test]
fn cancellation_uses_same_port_without_closing_or_relaunching_other_copy() -> Result<()> {
    let boundary = Boundary::default();
    let mut service = Service::new(boundary.clone());
    service.clock = PausedClock::new();
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
