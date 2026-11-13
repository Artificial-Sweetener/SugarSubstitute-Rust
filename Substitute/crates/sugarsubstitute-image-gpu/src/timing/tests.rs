// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify timing aggregation and outstanding custody with explicit durations and no GPU or sleeps.
use super::*;
use sugarsubstitute_contracts::execution::MemoryRelease;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn diagnostics() -> Result<GpuDiagnostics, Box<dyn std::error::Error>> {
    Ok(GpuDiagnostics::new(1, 2, &MemoryLease::new(16 * 1024, Box::new(Release)))?)
}

#[test]
fn windows_preserve_inflight_calls_across_reports_without_recounting_completions()
-> Result<(), Box<dyn std::error::Error>> {
    let mut window = Window::default();
    window.samples[Phase::Wait as usize].begin(Duration::from_millis(100));
    assert!(window.take(Duration::from_millis(999), false).is_none());
    let (first, elapsed) =
        window.take(Duration::from_secs(1), false).ok_or("Missing due timing window")?;
    assert_eq!(elapsed, Duration::from_secs(1));
    assert_eq!(first[Phase::Wait as usize].active, 1);
    assert_eq!(first[Phase::Wait as usize].busy_since, Duration::from_millis(100));
    let sample = &mut window.samples[Phase::Wait as usize];
    sample.finish(Duration::from_millis(1300), 1024, 2, 3);
    let (second, _) =
        window.take(Duration::from_secs(2), false).ok_or("Missing due timing window")?;
    let observed = second[Phase::Wait as usize];
    assert_eq!((observed.count, observed.active, observed.peak_active), (1, 0, 1));
    assert_eq!((observed.total_us, observed.maximum_us), (1_300_000, 1_300_000));
    assert_eq!((observed.bytes, observed.items, observed.timeouts), (1024, 2, 3));
    let (last, _) =
        window.take(Duration::from_millis(2001), true).ok_or("Missing due timing window")?;
    assert_eq!(last[Phase::Wait as usize].count, 0);
    assert_eq!(last[Phase::Wait as usize].peak_active, 0);
    Ok(())
}

#[test]
fn workload_counters_saturate_and_each_phase_keeps_independent_maxima() {
    let mut window = Window::default();
    let sample = &mut window.samples[Phase::Batch as usize];
    sample.begin(Duration::ZERO);
    sample.begin(Duration::from_millis(2));
    sample.finish(Duration::MAX, u64::MAX, u64::MAX, u64::MAX);
    sample.finish(Duration::from_micros(7), 1, 1, 1);
    assert_eq!(
        (sample.total_us, sample.maximum_us, sample.bytes, sample.items, sample.timeouts),
        (u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX)
    );
    assert_eq!((sample.active, sample.peak_active, sample.count), (0, 2, 2));
    assert_eq!(window.samples[Phase::Submit as usize].count, 0);
}

#[test]
fn ui_reporting_skips_bookkeeping_contention_without_losing_active_or_completed_work()
-> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = diagnostics()?;
    let mut span = diagnostics.begin(Phase::Present, 24, 1);
    span.timeout();
    {
        let mut window =
            diagnostics.0.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        window.last_report = Duration::MAX;
        assert_eq!(window.samples[Phase::Present as usize].active, 1);
        diagnostics.report();
    }
    drop(span);
    let window = diagnostics.0.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let sample = window.samples[Phase::Present as usize];
    assert_eq!((sample.active, sample.count, sample.timeouts, sample.bytes), (0, 1, 1, 24));
    Ok(())
}

#[test]
fn outstanding_span_retains_observations_until_terminal_return()
-> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = diagnostics()?;
    let weak = Arc::downgrade(&diagnostics.0);
    let span = diagnostics.begin(Phase::Acquire, 0, 1);
    drop(diagnostics);
    assert_eq!(weak.strong_count(), 1);
    drop(span);
    assert_eq!(weak.strong_count(), 0);
    Ok(())
}
