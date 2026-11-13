// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;

const TOTAL: u64 = 64 * GIB;

fn at(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}

fn sample(available: u64, footprint: u64) -> SystemMemorySample {
    SystemMemorySample {
        total_bytes: TOTAL,
        available_bytes: available,
        commit_available_bytes: Some(64 * GIB),
        os_pressure: OsPressure::None,
        footprint_bytes: footprint,
    }
}

fn native(bytes: u64) -> CacheCharge {
    CacheCharge { native_bytes: bytes, ..CacheCharge::default() }
}

#[test]
fn the_default_ceiling_is_a_quarter_of_physical_memory_and_fixed_ceilings_are_clamped() {
    assert_eq!(MemoryPreference::default().ceiling(TOTAL), 16 * GIB);
    assert_eq!(MemoryPreference::Share(5).ceiling(TOTAL), percent_of(TOTAL, 10));
    assert_eq!(MemoryPreference::Share(90).ceiling(TOTAL), 48 * GIB);
    assert_eq!(MemoryPreference::Fixed(8 * GIB).ceiling(TOTAL), 8 * GIB);
    assert_eq!(MemoryPreference::Fixed(MIB).ceiling(TOTAL), GIB);
    assert_eq!(MemoryPreference::Fixed(100 * GIB).ceiling(TOTAL), 48 * GIB);
    assert_eq!(MemoryPreference::largest_ceiling(TOTAL), 48 * GIB);
}

#[test]
fn plentiful_memory_grants_the_ceiling_minus_the_applications_other_footprint() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    // 3 GiB resident, 1 GiB of it already cached work.
    let decision = budget.observe(sample(40 * GIB, 3 * GIB), native(GIB), at(0));
    assert_eq!(decision.level, PressureLevel::Normal);
    assert_eq!(decision.ceiling_bytes, 16 * GIB);
    assert_eq!(decision.cache_bytes, 14 * GIB);
    assert_eq!(decision.native_bytes, 14 * GIB);
    assert_eq!(budget.sample_interval(), Duration::from_secs(2));
}

#[test]
fn caches_only_grow_into_memory_above_the_system_reserve() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    // The reserve is 4 GiB; available memory 9 GiB leaves 5 GiB to grow into.
    let decision = budget.observe(sample(9 * GIB, 2 * GIB), native(GIB), at(0));
    assert_eq!(decision.level, PressureLevel::Normal);
    assert_eq!(decision.cache_bytes, 6 * GIB);
}

#[test]
fn a_footprint_past_the_ceiling_leaves_the_floor() {
    let mut budget = MemoryBudget::new(MemoryPreference::Fixed(2 * GIB));
    let decision = budget.observe(sample(40 * GIB, 5 * GIB), native(0), at(0));
    assert_eq!(decision.cache_bytes, FLOOR_BYTES);
}

#[test]
fn pressure_sheds_by_level_and_critical_keeps_only_pinned_work() {
    let mut elevated = MemoryBudget::new(MemoryPreference::default());
    // Below twice the 4 GiB reserve. Base is min(16 - 2, 12 + 3) = 14 GiB; keep 85% of it.
    let decision = elevated.observe(sample(7 * GIB, 14 * GIB), native(12 * GIB), at(0));
    assert_eq!(decision.level, PressureLevel::Elevated);
    assert_eq!(decision.cache_bytes, percent_of(14 * GIB, 85));
    assert_eq!(elevated.sample_interval(), Duration::from_millis(500));

    let charged = native(10 * GIB);
    let footprint = 12 * GIB;
    let mut high = MemoryBudget::new(MemoryPreference::default());
    // Nothing is spare below the reserve, so base is the 10 GiB held; High keeps half.
    let decision = high.observe(sample(3 * GIB, footprint), charged, at(0));
    assert_eq!(decision.level, PressureLevel::High);
    assert_eq!(decision.cache_bytes, 5 * GIB);

    let mut critical = MemoryBudget::new(MemoryPreference::default());
    let decision = critical.observe(sample(GIB, footprint), charged, at(0));
    assert_eq!(decision.level, PressureLevel::Critical);
    assert_eq!(decision.cache_bytes, 0);
    assert_eq!(decision.native_bytes, 0);
}

#[test]
fn elevated_pressure_stops_growth_without_evicting_small_caches() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    let decision = budget.observe(sample(7 * GIB, 2 * GIB), native(GIB), at(0));
    assert_eq!(decision.level, PressureLevel::Elevated);
    assert_eq!(decision.cache_bytes, GIB);
}

#[test]
fn operating_system_signals_and_commit_exhaustion_escalate_pressure() {
    let mut signalled = sample(40 * GIB, GIB);
    signalled.os_pressure = OsPressure::High;
    assert_eq!(
        MemoryBudget::new(MemoryPreference::default()).observe(signalled, native(0), at(0)).level,
        PressureLevel::High
    );
    let mut warned = sample(40 * GIB, GIB);
    warned.os_pressure = OsPressure::Elevated;
    assert_eq!(
        MemoryBudget::new(MemoryPreference::default()).observe(warned, native(0), at(0)).level,
        PressureLevel::Elevated
    );
    let mut committed = sample(40 * GIB, GIB);
    committed.commit_available_bytes = Some(GIB);
    assert_eq!(
        MemoryBudget::new(MemoryPreference::default()).observe(committed, native(0), at(0)).level,
        PressureLevel::Critical
    );
}

#[test]
fn pressure_escalates_at_once_but_relaxes_only_after_ten_calm_seconds() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    assert_eq!(budget.observe(sample(GIB, GIB), native(0), at(0)).level, PressureLevel::Critical);
    assert_eq!(
        budget.observe(sample(40 * GIB, GIB), native(0), at(1)).level,
        PressureLevel::Critical
    );
    assert_eq!(
        budget.observe(sample(40 * GIB, GIB), native(0), at(10)).level,
        PressureLevel::Critical
    );
    // A renewed pressure spike restarts the calm period.
    assert_eq!(budget.observe(sample(GIB, GIB), native(0), at(10)).level, PressureLevel::Critical);
    assert_eq!(
        budget.observe(sample(40 * GIB, GIB), native(0), at(12)).level,
        PressureLevel::Critical
    );
    assert_eq!(
        budget.observe(sample(40 * GIB, GIB), native(0), at(22)).level,
        PressureLevel::Normal
    );
}

#[test]
fn the_embedded_frontend_receives_at_most_a_quarter_of_the_allowance_and_one_gibibyte() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    let charge = CacheCharge { native_bytes: 0, page_bytes: 0, page_demand_bytes: 512 * MIB };
    let decision = budget.observe(sample(40 * GIB, GIB), charge, at(0));
    assert_eq!(decision.page_bytes, 512 * MIB);
    assert_eq!(decision.native_bytes, decision.cache_bytes - 512 * MIB);
    let greedy = CacheCharge { page_demand_bytes: 8 * GIB, ..charge };
    assert_eq!(budget.observe(sample(40 * GIB, GIB), greedy, at(1)).page_bytes, GIB);
    let mut small = MemoryBudget::new(MemoryPreference::Fixed(2 * GIB));
    let decision = small.observe(sample(40 * GIB, GIB), greedy, at(0));
    assert_eq!(decision.page_bytes, decision.cache_bytes / 4);
}

#[test]
fn changing_the_preference_applies_to_the_next_decision() {
    let mut budget = MemoryBudget::new(MemoryPreference::default());
    budget.set_preference(MemoryPreference::Fixed(4 * GIB));
    assert_eq!(budget.preference(), MemoryPreference::Fixed(4 * GIB));
    assert_eq!(budget.observe(sample(40 * GIB, GIB), native(0), at(0)).ceiling_bytes, 4 * GIB);
}

#[cfg(windows)]
#[test]
fn the_windows_sampler_reports_physical_memory_and_this_process_footprint() -> std::io::Result<()> {
    use super::SystemMemory as _;
    let sampler = SystemMemorySampler::new()?;
    // A process identity that cannot exist contributes nothing rather than failing.
    let sample = sampler.sample(&[u32::MAX])?;
    assert!(sample.total_bytes > 0);
    assert!(sample.available_bytes <= sample.total_bytes);
    assert!(sample.footprint_bytes > 0);
    assert!(sample.commit_available_bytes.is_some());
    Ok(())
}
