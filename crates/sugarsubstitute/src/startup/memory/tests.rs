// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_execution::memory_budget::{PressureLevel, SystemMemorySample};

const GIB: u64 = 1024 * 1024 * 1024;

/// Scripted samples that record which processes each sample measured.
#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<(SystemMemorySample, Vec<Vec<u32>>)>>);

impl Scripted {
    fn set_available(&self, bytes: u64) {
        if let Ok(mut state) = self.0.lock() {
            state.0.available_bytes = bytes;
        }
    }
}

impl SystemMemory for Scripted {
    fn sample(&self, processes: &[u32]) -> io::Result<SystemMemorySample> {
        let mut state = self.0.lock().map_err(|_| io::Error::other("poisoned"))?;
        state.1.push(processes.to_vec());
        Ok(state.0)
    }
}

fn scripted() -> Scripted {
    let source = Scripted::default();
    if let Ok(mut state) = source.0.lock() {
        state.0 = SystemMemorySample {
            total_bytes: 64 * GIB,
            available_bytes: 40 * GIB,
            commit_available_bytes: Some(64 * GIB),
            footprint_bytes: GIB,
            ..SystemMemorySample::default()
        };
    }
    source
}

#[test]
fn the_governor_samples_when_due_and_reports_only_changed_decisions() -> Result<(), Box<dyn Error>>
{
    let source = scripted();
    let mut governor = MemoryGovernor::new(Box::new(source.clone()), MemoryPreference::default());
    assert_eq!(governor.total_bytes()?, 64 * GIB);
    let start = governor.next_deadline();
    let first = governor.poll(start, Moment::from_nanos(0), CacheCharge::default())?;
    assert_eq!(first.map(|decision| decision.level), Some(PressureLevel::Normal));
    assert_eq!(governor.next_deadline(), start + Duration::from_secs(2));
    // Not yet due: no sample is taken.
    assert!(
        governor
            .poll(start + Duration::from_secs(1), Moment::from_nanos(1), CacheCharge::default())?
            .is_none()
    );
    // Due again with nothing changed: sampled, but no new decision.
    let unchanged = governor.poll(
        start + Duration::from_secs(2),
        Moment::from_nanos(2),
        CacheCharge::default(),
    )?;
    assert!(unchanged.is_none());
    source.set_available(GIB);
    let pressed = governor.poll(
        start + Duration::from_secs(4),
        Moment::from_nanos(4),
        CacheCharge::default(),
    )?;
    assert_eq!(
        pressed.map(|decision| (decision.level, decision.cache_bytes)),
        Some((PressureLevel::Critical, 0))
    );
    // Pressure samples more often.
    assert_eq!(
        governor.next_deadline(),
        start + Duration::from_secs(4) + Duration::from_millis(500)
    );
    Ok(())
}
