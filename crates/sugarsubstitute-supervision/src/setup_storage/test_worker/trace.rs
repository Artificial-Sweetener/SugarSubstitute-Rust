// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe synchronous storage spans per admitted test job without a global subscriber.
use super::observation::Observation;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{
    Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

#[derive(Default)]
struct State {
    next: u64,
    omitted: u64,
    spans: BTreeMap<u64, (&'static str, usize)>,
    active: Vec<(u64, &'static str, Instant)>,
    completed: BTreeMap<&'static str, (u64, Duration, Duration)>,
}

/// Bounded diagnostic state shared only by one worker and its completion observer.
#[derive(Clone, Default)]
pub(crate) struct StorageTrace(Arc<Mutex<State>>, Option<Observation>);
impl StorageTrace {
    /// Attach bounded native-return observation to this job's isolated diagnostic subscriber.
    pub(crate) fn new(observation: Observation) -> Self {
        Self(Arc::default(), Some(observation))
    }
    /// Report live calls and aggregate elapsed time; neither affects operation clocks or admission.
    pub(crate) fn snapshot(&self) -> String {
        let state = crate::registry::lock(&self.0);
        let active: Vec<_> = state
            .active
            .iter()
            .map(|(_, name, since)| (*name, since.elapsed().as_millis()))
            .collect();
        let completed: Vec<_> = state
            .completed
            .iter()
            .map(|(name, (count, elapsed, maximum))| {
                (*name, *count, elapsed.as_millis(), maximum.as_millis())
            })
            .collect();
        format!(
            "active_ms={active:?}, completed_count_total_max_ms={completed:?}, omitted={}",
            state.omitted
        )
    }
}

impl Subscriber for StorageTrace {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "sugarsubstitute::storage"
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut state = crate::registry::lock(&self.0);
        state.next += 1;
        let id = state.next;
        if state.spans.len() < 32 {
            state.spans.insert(id, (attributes.metadata().name(), 1));
        } else {
            state.omitted += 1;
        }
        Id::from_u64(id)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &tracing::Event<'_>) {}
    fn enter(&self, id: &Id) {
        let mut state = crate::registry::lock(&self.0);
        if state.active.len() < 32
            && let Some((name, _)) = state.spans.get(&id.into_u64()).copied()
        {
            state.active.push((id.into_u64(), name, Instant::now()));
        } else {
            state.omitted += 1;
        }
    }
    fn exit(&self, id: &Id) {
        let mut state = crate::registry::lock(&self.0);
        if let Some(index) =
            state.active.iter().rposition(|(active, _, _)| *active == id.into_u64())
        {
            let (_, name, since) = state.active.remove(index);
            if state.completed.len() < 32 || state.completed.contains_key(name) {
                let elapsed = since.elapsed();
                let entry = state.completed.entry(name).or_default();
                entry.0 += 1;
                entry.1 += elapsed;
                entry.2 = entry.2.max(elapsed);
            } else {
                state.omitted += 1;
            }
            drop(state);
            if let Some(observation) = &self.1 {
                observation.returned();
            }
        }
    }
    fn clone_span(&self, id: &Id) -> Id {
        if let Some((_, references)) = crate::registry::lock(&self.0).spans.get_mut(&id.into_u64())
        {
            *references += 1;
        }
        id.clone()
    }
    fn try_close(&self, id: Id) -> bool {
        let mut state = crate::registry::lock(&self.0);
        if let Some((_, references)) = state.spans.get_mut(&id.into_u64()) {
            *references -= 1;
            if *references != 0 {
                return false;
            }
        }
        state.spans.remove(&id.into_u64());
        true
    }
}

#[test]
fn storage_diagnostics_follow_nested_custody_and_ignore_other_targets() {
    let trace = StorageTrace::default();
    tracing::subscriber::with_default(trace.clone(), || {
        let outer = tracing::trace_span!(target: "sugarsubstitute::storage", "outer").entered();
        let unrelated = tracing::trace_span!(target: "unrelated", "other").entered();
        {
            let inner = tracing::trace_span!(target: "sugarsubstitute::storage", "inner");
            let cloned = inner.clone();
            let _entered = cloned.enter();
            let state = crate::registry::lock(&trace.0);
            assert_eq!(
                state.active.iter().map(|(_, name, _)| *name).collect::<Vec<_>>(),
                ["outer", "inner"]
            );
        }
        drop(unrelated);
        drop(outer);
    });
    let state = crate::registry::lock(&trace.0);
    assert!(state.active.is_empty());
    assert!(state.spans.is_empty());
    assert_eq!(state.completed["inner"].0, 1);
    assert_eq!(state.completed["outer"].0, 1);
    assert_eq!(state.completed.len(), 2);
}

#[test]
fn only_actual_storage_returns_advance_native_observation_without_completion_traffic()
-> std::io::Result<()> {
    let wake = Arc::new(super::Wake::default());
    let observation = Observation::new(wake.clone());
    observation.started();
    let trace = StorageTrace::new(observation.clone());
    tracing::subscriber::with_default(trace, || {
        let outer = tracing::trace_span!(target: "sugarsubstitute::storage", "outer").entered();
        {
            let _unrelated = tracing::trace_span!(target: "other", "unrelated").entered();
        }
        assert_eq!(observation.progress().map(|progress| progress.returned).ok(), Some(0));
        {
            let _inner =
                tracing::trace_span!(target: "sugarsubstitute::storage", "inner").entered();
        }
        assert_eq!(observation.progress().map(|progress| progress.returned).ok(), Some(1));
        drop(outer);
    });
    assert_eq!(observation.progress()?.returned, 2);
    assert_eq!(*crate::registry::lock(&wake.0), 1);
    observation.completed()
}
