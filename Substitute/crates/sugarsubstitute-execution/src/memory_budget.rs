// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Size the application's reconstructible caches from the user's ceiling and system pressure.
//!
//! The policy is pure: callers supply measured system samples and the bytes their caches
//! currently hold, and receive the allowance each cache family may keep. Caches grow only
//! into memory the system can spare above its reserve and give memory back as pressure rises.

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::SystemMemorySampler;

use std::time::Duration;
use sugarsubstitute_contracts::execution::Moment;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
/// Default share of physical memory the whole application may occupy.
pub const DEFAULT_SHARE_PERCENT: u8 = 25;
/// Smallest and largest user-selectable shares of physical memory.
pub const SHARE_PERCENT_RANGE: std::ops::RangeInclusive<u8> = 10..=75;
/// Smallest user-selectable fixed ceiling.
pub const MINIMUM_FIXED_BYTES: u64 = GIB;
/// Retained work kept outside Critical pressure: the visible document and its predecessor.
pub const FLOOR_BYTES: u64 = 256 * MIB;
/// Largest share of the cache allowance granted to the embedded frontend's own caches.
pub const PAGE_SHARE_LIMIT_BYTES: u64 = GIB;
const RESERVE_CAP_BYTES: u64 = 4 * GIB;
const RELAXATION: Duration = Duration::from_secs(10);
const NORMAL_INTERVAL: Duration = Duration::from_secs(2);
const PRESSURED_INTERVAL: Duration = Duration::from_millis(500);

/// The user's choice of application memory ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryPreference {
    /// A percentage of physical memory, clamped to [`SHARE_PERCENT_RANGE`].
    Share(u8),
    /// A fixed byte ceiling, clamped between [`MINIMUM_FIXED_BYTES`] and 75% of physical memory.
    Fixed(u64),
}

impl Default for MemoryPreference {
    fn default() -> Self {
        Self::Share(DEFAULT_SHARE_PERCENT)
    }
}

impl MemoryPreference {
    /// Resolve the application ceiling in bytes for a machine with `total_bytes` of RAM.
    pub fn ceiling(self, total_bytes: u64) -> u64 {
        let largest = percent_of(total_bytes, *SHARE_PERCENT_RANGE.end());
        match self {
            Self::Share(percent) => percent_of(
                total_bytes,
                percent.clamp(*SHARE_PERCENT_RANGE.start(), *SHARE_PERCENT_RANGE.end()),
            ),
            Self::Fixed(bytes) => bytes.min(largest).max(MINIMUM_FIXED_BYTES.min(largest)),
        }
    }

    /// The largest ceiling any preference can select on this machine.
    pub fn largest_ceiling(total_bytes: u64) -> u64 {
        percent_of(total_bytes, *SHARE_PERCENT_RANGE.end())
    }
}

/// The operating system's own low-memory signal, where the platform provides one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum OsPressure {
    /// No signal, or the platform reports memory as plentiful.
    #[default]
    None,
    /// The platform reports sustained reclaim activity or a warning level.
    Elevated,
    /// The platform reports memory as low (Windows low-memory resource notification).
    High,
}

/// One measurement of system memory and of the application's own resident footprint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemMemorySample {
    /// Installed physical memory.
    pub total_bytes: u64,
    /// Physical memory the system can hand out without paging.
    pub available_bytes: u64,
    /// Remaining commit charge before the system commit limit, where reported.
    pub commit_available_bytes: Option<u64>,
    /// The platform's own pressure signal.
    pub os_pressure: OsPressure,
    /// Private bytes of every application process, including the embedded browser's.
    pub footprint_bytes: u64,
}

/// A source of system memory samples: the platform sampler, or scripted samples in tests.
pub trait SystemMemory {
    /// Sample system memory and the private bytes of this process plus `processes`.
    ///
    /// # Errors
    /// Preserves the platform error when system memory cannot be read.
    fn sample(&self, processes: &[u32]) -> std::io::Result<SystemMemorySample>;
}

/// Bytes the application's reconstructible caches hold right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheCharge {
    /// Retained native work (parked canvas documents and their decoded sources).
    pub native_bytes: u64,
    /// Caches the embedded frontend reports holding.
    pub page_bytes: u64,
    /// Bytes the embedded frontend would hold without any limit.
    pub page_demand_bytes: u64,
}

impl CacheCharge {
    fn total(self) -> u64 {
        self.native_bytes.saturating_add(self.page_bytes)
    }
}

/// How hard the system is pressed for memory, from plentiful to nearly exhausted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressureLevel {
    /// Caches may grow to their allowance.
    #[default]
    Normal,
    /// Caches stop growing and shed to 85% of their allowance.
    Elevated,
    /// Caches shed to half their allowance.
    High,
    /// Caches keep only pinned work.
    Critical,
}

impl PressureLevel {
    /// The stable lowercase name used in logs and host protocols.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Elevated => "elevated",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// The allowance each cache family may hold until the next decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BudgetDecision {
    /// Current pressure after hysteresis.
    pub level: PressureLevel,
    /// The application ceiling resolved from the preference.
    pub ceiling_bytes: u64,
    /// Total bytes all reconstructible caches may hold.
    pub cache_bytes: u64,
    /// Bytes native retained work may hold.
    pub native_bytes: u64,
    /// Bytes the embedded frontend's caches may hold.
    pub page_bytes: u64,
}

/// Turn successive samples into cache allowances, escalating at once and relaxing slowly.
#[derive(Debug)]
pub struct MemoryBudget {
    preference: MemoryPreference,
    level: PressureLevel,
    relaxing_since: Option<Moment>,
}

impl MemoryBudget {
    /// Start at Normal pressure with the supplied preference.
    pub const fn new(preference: MemoryPreference) -> Self {
        Self { preference, level: PressureLevel::Normal, relaxing_since: None }
    }

    /// The preference currently applied to ceilings.
    pub const fn preference(&self) -> MemoryPreference {
        self.preference
    }

    /// Apply a new preference; the next decision uses it.
    pub fn set_preference(&mut self, preference: MemoryPreference) {
        self.preference = preference;
    }

    /// How long to wait before the next sample at the current pressure.
    pub fn sample_interval(&self) -> Duration {
        if self.level == PressureLevel::Normal { NORMAL_INTERVAL } else { PRESSURED_INTERVAL }
    }

    /// Decide every allowance from one sample taken at `now`.
    pub fn observe(
        &mut self,
        sample: SystemMemorySample,
        charge: CacheCharge,
        now: Moment,
    ) -> BudgetDecision {
        self.advance(classify(sample), now);
        let ceiling = self.preference.ceiling(sample.total_bytes);
        let charged = charge.total();
        // What the application occupies besides its caches is not reclaimable here.
        let fixed = sample.footprint_bytes.saturating_sub(charged);
        let reserve = reserve_bytes(sample.total_bytes);
        let spare = sample.available_bytes.saturating_sub(reserve);
        let affordable = charged.saturating_add(spare);
        let base = ceiling.saturating_sub(fixed).min(affordable).max(FLOOR_BYTES);
        let cache = match self.level {
            PressureLevel::Normal => base,
            PressureLevel::Elevated => charged.min(percent_of(base, 85)).max(FLOOR_BYTES),
            PressureLevel::High => charged.min(base / 2).max(FLOOR_BYTES),
            PressureLevel::Critical => 0,
        };
        let page = charge.page_demand_bytes.min(cache / 4).min(PAGE_SHARE_LIMIT_BYTES);
        BudgetDecision {
            level: self.level,
            ceiling_bytes: ceiling,
            cache_bytes: cache,
            native_bytes: cache - page,
            page_bytes: page,
        }
    }

    fn advance(&mut self, measured: PressureLevel, now: Moment) {
        if measured >= self.level {
            self.level = measured;
            self.relaxing_since = None;
            return;
        }
        let since = *self.relaxing_since.get_or_insert(now);
        if now.checked_duration_since(since).is_ok_and(|calm| calm >= RELAXATION) {
            self.level = measured;
            self.relaxing_since = None;
        }
    }
}

/// Memory the system keeps for itself and other applications: 10% of RAM, at most 4 GiB.
pub fn reserve_bytes(total_bytes: u64) -> u64 {
    (total_bytes / 10).min(RESERVE_CAP_BYTES)
}

fn classify(sample: SystemMemorySample) -> PressureLevel {
    let reserve = reserve_bytes(sample.total_bytes);
    let available = sample.available_bytes;
    let commit_exhausted = sample.commit_available_bytes.is_some_and(|commit| commit < reserve);
    if available < reserve / 2 || commit_exhausted {
        PressureLevel::Critical
    } else if available < reserve || sample.os_pressure == OsPressure::High {
        PressureLevel::High
    } else if available < reserve.saturating_mul(2) || sample.os_pressure == OsPressure::Elevated {
        PressureLevel::Elevated
    } else {
        PressureLevel::Normal
    }
}

fn percent_of(bytes: u64, percent: u8) -> u64 {
    // Widening keeps the product exact; a percentage at most 100 cannot exceed the input.
    u64::try_from(u128::from(bytes) * u128::from(percent) / 100).unwrap_or(bytes)
}

#[cfg(test)]
#[path = "memory_budget_tests.rs"]
mod tests;
