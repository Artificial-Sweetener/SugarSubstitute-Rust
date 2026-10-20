// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own a monotonic overall estimate and finite, evidence-triggered activity sweeps.
use super::StartupPhase;
use std::time::Duration;
use sugarsubstitute_contracts::startup::StartupOperation;

const SWEEP: Duration = Duration::from_millis(1200);

/// Per-attempt presentation policy; estimates never grant lifecycle authority.
/// Stage weights describe the whole journey, not a percentage of elapsed time.
/// Unknown output signals activity only. Silence cannot increase the target.
pub struct StartupProgress {
    operation: StartupOperation,
    target: f32,
    displayed: f32,
    sampled: Duration,
    pulse: Option<Duration>,
    last_activity: Option<Duration>,
    phase: StartupPhase,
}
impl Default for StartupProgress {
    fn default() -> Self {
        Self {
            operation: StartupOperation::Preparing,
            target: 0.0,
            displayed: 0.0,
            sampled: Duration::ZERO,
            pulse: None,
            last_activity: None,
            phase: StartupPhase::SplashFirstFrame,
        }
    }
}
impl StartupProgress {
    /// Record actual output activity and optionally a recognized operation.
    /// Late or repeated milestones cannot reset the overall fill; operation text
    /// may return to dependency setup when an extension performs additional work.
    pub fn output(&mut self, operation: Option<StartupOperation>, now: Duration) {
        if self.phase != StartupPhase::PreparingConnection {
            return;
        }
        if let Some(operation) = operation {
            let target: f32 = match operation {
                StartupOperation::Preparing => 0.08,
                StartupOperation::Dependencies => 0.18,
                StartupOperation::Runtime => 0.36,
                StartupOperation::Device => 0.46,
                StartupOperation::CustomNodes => 0.56,
                StartupOperation::Database => 0.68,
                StartupOperation::Server => 0.74,
                _ => return,
            };
            self.operation = operation;
            self.target = self.target.max(target);
        }
        self.activity(now);
    }
    fn activity(&mut self, now: Duration) {
        self.last_activity = Some(now);
        if self.pulse.is_none_or(|start| now.saturating_sub(start) >= SWEEP) {
            self.pulse = Some(now);
        }
    }
    /// Observe the authoritative phase, then ease only toward already-observed work.
    /// A maximum one-second interpolation softens sparse milestone jumps; no timer
    /// creates new work. Only acknowledged native reveal completes the bar.
    pub fn sample(&mut self, phase: StartupPhase, now: Duration, reduced_motion: bool) {
        if phase != self.phase {
            let milestone = match phase {
                StartupPhase::PreparingConnection => Some((StartupOperation::Preparing, 0.08)),
                StartupPhase::ServerReady => Some((StartupOperation::Frontend, 0.80)),
                StartupPhase::WebviewLoading => Some((StartupOperation::Frontend, 0.88)),
                StartupPhase::FrontendReady | StartupPhase::RevealingMain => {
                    Some((StartupOperation::Reveal, 0.97))
                }
                StartupPhase::Running => Some((StartupOperation::Ready, 1.0)),
                _ => None,
            };
            if let Some((operation, target)) = milestone {
                self.operation = operation;
                self.target = self.target.max(target);
                self.activity(now);
            }
            self.phase = phase;
        }
        if matches!(
            phase,
            StartupPhase::Failed
                | StartupPhase::Cancelling
                | StartupPhase::Stopped
                | StartupPhase::Running
        ) {
            self.pulse = None;
            self.last_activity = None;
        } else if let Some(start) = self.pulse
            && now.saturating_sub(start) >= SWEEP
        {
            self.pulse = self
                .last_activity
                .filter(|last| *last > start && now.saturating_sub(*last) < SWEEP);
        }
        self.displayed = if reduced_motion || phase == StartupPhase::Running {
            self.target
        } else {
            (self.displayed + now.saturating_sub(self.sampled).as_secs_f32()).min(self.target)
        };
        self.sampled = now;
    }
    /// Latest observed operation; independent of the estimate's high-water mark.
    pub const fn operation(&self) -> StartupOperation {
        self.operation
    }
    /// Overall completion estimate in the inclusive range zero through one.
    pub const fn completed(&self) -> f32 {
        self.displayed
    }
    /// One left-to-right sweep; no activity is fabricated during a silent wait.
    pub fn activity_phase(&self, now: Duration) -> Option<f32> {
        self.pulse.and_then(|start| {
            let age = now.saturating_sub(start);
            (age < SWEEP).then(|| age.as_secs_f32() / SWEEP.as_secs_f32())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_journey_preserves_progress_through_restarts_silence_and_failure() {
        let mut progress = StartupProgress::default();
        progress.sample(StartupPhase::PreparingConnection, Duration::ZERO, true);
        let mut previous = progress.completed();
        for (second, operation) in [
            (1, StartupOperation::Dependencies),
            (2, StartupOperation::Runtime),
            (3, StartupOperation::CustomNodes),
            (4, StartupOperation::Dependencies),
            (5, StartupOperation::Server),
        ] {
            progress.output(Some(operation), Duration::from_secs(second));
            progress.sample(StartupPhase::PreparingConnection, Duration::from_secs(second), true);
            assert!(progress.completed() >= previous && progress.completed() < 0.80);
            assert_eq!(progress.operation(), operation);
            previous = progress.completed();
        }
        progress.sample(StartupPhase::PreparingConnection, Duration::from_secs(7200), true);
        assert_eq!(progress.completed(), previous);
        assert_eq!(progress.activity_phase(Duration::from_secs(7200)), None);
        progress.sample(StartupPhase::Failed, Duration::from_secs(7201), true);
        assert_eq!(progress.completed(), previous);
    }
    #[test]
    fn pulses_finish_during_bursts_and_completion_requires_native_reveal() {
        let mut progress = StartupProgress::default();
        progress.sample(StartupPhase::PreparingConnection, Duration::ZERO, false);
        for millis in [200, 400, 600, 800, 1000] {
            let now = Duration::from_millis(millis);
            progress.output(None, now);
            progress.sample(StartupPhase::PreparingConnection, now, false);
            assert!(
                progress
                    .activity_phase(now)
                    .is_some_and(|phase| (phase - millis as f32 / 1200.0).abs() < 0.000001)
            );
        }
        progress.sample(StartupPhase::PreparingConnection, Duration::from_secs(3), false);
        assert_eq!(progress.activity_phase(Duration::from_secs(3)), None);
        assert_eq!(progress.completed(), 0.08);
        for phase in [
            StartupPhase::ServerReady,
            StartupPhase::WebviewLoading,
            StartupPhase::FrontendReady,
            StartupPhase::RevealingMain,
        ] {
            progress.sample(phase, Duration::from_secs(4), true);
            assert!(progress.completed() < 1.0);
        }
        progress.sample(StartupPhase::Running, Duration::from_secs(4), true);
        assert_eq!(progress.completed(), 1.0);
        assert_eq!(progress.activity_phase(Duration::from_secs(4)), None);
    }
}
