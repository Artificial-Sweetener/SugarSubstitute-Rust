// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real native request admission, original deadlines and checked deferral retirement.
mod native;
use super::*;
use native::{Witness, boundaries};
use std::error::Error;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    desktop_session::wire::SessionBinding,
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
struct Fixture {
    media: Media,
    memory: MemoryLease,
}
impl Fixture {
    fn new() -> Result<Self> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../Sugar/integrations/contracts/session_vectors.json"
        ))?;
        let bootstrap = SessionBootstrap::new(
            serde_json::from_value(vectors["scope"].clone())?,
            LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
            "00000000-0000-4000-8000-000000000001".into(),
        )?;
        let identity = MainIdentity {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        };
        let memory = MemoryLease::new(24 * 1024 * 1024, Box::new(Release));
        Ok(Self {
            media: Media::new(identity, bootstrap, Events::new(&memory, || {})?, &memory)?,
            memory,
        })
    }
    fn defer(&self, now: Instant, head: bool, witness: Rc<Witness>) -> Result<bool> {
        let requested = Requested {
            output: "00000000-0000-4000-8000-000000000002".into(),
            binding: SessionBinding {
                entry_id: "00000000-0000-4000-8000-000000000003".into(),
                runtime_session: "current runtime".into(),
                generation: 1,
                graph_revision: 2,
                source_revision: 3,
                navigation_revision: 4,
            },
            range: None,
            head,
        };
        let (args, environment) = boundaries(witness);
        Ok(self.media.defer(
            requested,
            self.memory.claim(REQUEST_BYTES)?,
            &args,
            &environment,
            now,
        )?)
    }
    fn request(&self, now: Instant) -> Result<SessionMediaObservation> {
        match self.media.step(now)?.observation {
            Some(DesktopStartupObservation::SessionMediaRequested(value)) => Ok(value),
            _ => Err("Expected media job admission".into()),
        }
    }
    fn retire(&self, now: Instant) -> Result {
        self.media.close();
        for _ in 0..CAPACITY * 2 {
            if self.media.retired() {
                return Ok(());
            }
            assert!(self.media.step(now)?.consumed);
        }
        Err("Media registry did not settle its bounded native obligations".into())
    }
}
#[test]
fn two_active_eight_queued_and_original_cutoff_are_enforced_by_the_real_registry() -> Result {
    let fixture = Fixture::new()?;
    let now = Instant::now();
    assert!(fixture.defer(now, true, Rc::new(Witness::default()))?);
    for _ in 1..CAPACITY {
        assert!(fixture.defer(now, false, Rc::new(Witness::default()))?);
    }
    assert!(!fixture.defer(now, false, Rc::new(Witness::default()))?);
    let first = fixture.request(now)?;
    let second = fixture.request(now)?;
    assert_eq!(first.deadline(), now + LIFETIME);
    assert_eq!(second.deadline(), now + LIFETIME);
    assert!(!fixture.media.step(now)?.consumed);
    let cutoff = now + LIFETIME;
    for request in [&first, &second] {
        match fixture.media.step(cutoff)?.observation {
            Some(DesktopStartupObservation::SessionMediaCancelled { identity, request_id }) => {
                assert_eq!(identity, request.identity());
                assert_eq!(request_id, request.request_id());
            }
            _ => return Err("Expected original-deadline cancellation".into()),
        }
    }
    fixture.media.close();
    for _ in 0..CAPACITY {
        assert!(fixture.media.step(cutoff)?.consumed);
    }
    assert!(fixture.media.retired());
    Ok(())
}
#[test]
fn held_get_keeps_later_head_progress_without_spinning_or_dispatching_another_get() -> Result {
    let fixture = Fixture::new()?;
    let now = Instant::now();
    assert!(fixture.defer(now, false, Rc::new(Witness::default()))?);
    let held = fixture.request(now)?;
    assert!(!held.head());
    assert!(fixture.defer(now, false, Rc::new(Witness::default()))?);
    assert!(!fixture.media.needs_progress(now));
    assert!(!fixture.media.step(now)?.consumed);
    assert!(fixture.defer(now, true, Rc::new(Witness::default()))?);
    assert!(fixture.media.needs_progress(now));
    let first_head = fixture.request(now)?;
    assert!(first_head.head());
    assert_eq!(first_head.deadline(), now + LIFETIME);
    assert!(fixture.defer(now, true, Rc::new(Witness::default()))?);
    assert!(!fixture.media.needs_progress(now));
    assert!(!fixture.media.step(now)?.consumed);
    fixture.media.reply(SessionMediaReply::metadata(
        first_head.identity(),
        first_head.request_id(),
        "image/webp",
        901,
        true,
    )?)?;
    assert!(fixture.media.step(now)?.consumed);
    let second_head = fixture.request(now)?;
    assert!(second_head.head());
    assert_ne!(first_head.request_id(), second_head.request_id());
    fixture.media.reply(SessionMediaReply::unavailable(
        held.identity(),
        held.request_id(),
        SessionMediaFailure::Unavailable,
    ))?;
    assert!(fixture.media.step(now)?.consumed);
    let next_get = fixture.request(now)?;
    assert!(!next_get.head());
    assert_ne!(held.request_id(), next_get.request_id());
    assert_eq!(next_get.deadline(), now + LIFETIME);
    assert!(!fixture.media.step(now)?.consumed);
    fixture.retire(now)
}
#[test]
fn saturated_get_deferrals_preserve_metadata_admission_within_original_total_credit() -> Result {
    let fixture = Fixture::new()?;
    let now = Instant::now();
    for _ in 0..GET_CAPACITY {
        assert!(fixture.defer(now, false, Rc::new(Witness::default()))?);
    }
    assert!(!fixture.defer(now, false, Rc::new(Witness::default()))?);
    assert!(fixture.defer(now, true, Rc::new(Witness::default()))?);
    assert!(!fixture.defer(now, true, Rc::new(Witness::default()))?);
    let head = fixture.request(now)?;
    assert!(head.head());
    let get = fixture.request(now)?;
    assert!(!get.head());
    assert!(!fixture.media.needs_progress(now));
    assert!(!fixture.media.step(now)?.consumed);
    fixture.retire(now)
}
#[test]
fn failed_complete_keeps_exact_deferral_and_does_not_assign_response_twice() -> Result {
    let fixture = Fixture::new()?;
    let now = Instant::now();
    let witness = Rc::new(Witness::default());
    witness.fail_complete.set(true);
    assert!(fixture.defer(now, true, witness.clone())?);
    let request = fixture.request(now)?;
    fixture.media.reply(SessionMediaReply::metadata(
        request.identity(),
        request.request_id(),
        "image/webp",
        901,
        true,
    )?)?;
    assert!(fixture.media.step(now).is_err());
    assert_eq!(witness.assigned.get(), 1);
    assert_eq!(witness.completed.get(), 1);
    assert!(fixture.media.step(now)?.consumed);
    assert_eq!(witness.assigned.get(), 1);
    assert_eq!(witness.completed.get(), 2);
    assert_eq!(witness.status.get(), 200);
    assert!(witness.headers.borrow().contains("Content-Length: 901"));
    assert!(witness.headers.borrow().contains("X-SugarSubstitute-Animated: 1"));
    fixture.media.close();
    assert!(fixture.media.retired());
    fixture.media.reply(SessionMediaReply::unavailable(
        request.identity(),
        request.request_id(),
        SessionMediaFailure::Unavailable,
    ))?;
    assert!(fixture.media.retired());
    Ok(())
}
