// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own mascot timing and weighted selection without scheduling, windowing, or startup state.

use std::time::Duration;

/// Count the existing numbered and guest splash character assets.
pub const POSE_COUNT: usize = 40;
const FLIP_MS: u64 = 390;
const RECENCY_MULTIPLIERS: [f64; 5] = [0.10, 0.20, 0.35, 0.55, 0.75];

/// Describe one immutable mascot presentation in the library's numeric-then-name order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationFrame {
    /// Index of the currently painted PNG, switching at the flip midpoint.
    pub pose: usize,
    /// Front-edge-front flip fraction from zero through one.
    pub progress: f64,
    /// Positive or negative direction of the card's slant.
    pub direction: i8,
}

impl AnimationFrame {
    /// Present the first numbered pose before animation or other resources are prepared.
    pub const fn bootstrap() -> Self {
        Self { pose: 0, progress: 0.0, direction: 1 }
    }
}

/// Identify the active timing phase independently of any GUI timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipPhase {
    /// Retain the committed pose until its sampled hold expires.
    Hold,
    /// Turn the card through the queued pose and commit at the end.
    Flip,
}

/// Advance source-compatible hold/flip behavior against an injected elapsed clock.
///
/// The caller drives `tick` from its event loop. A late tick advances one phase, matching
/// the source GUI timer instead of replaying missed frames. Seeded randomness is portable;
/// selection probabilities and recency semantics match the Python implementation, while
/// the random sequence is deliberately independent of Python's generator algorithm.
pub struct PaperFlip {
    phase: FlipPhase,
    phase_started: Duration,
    last_tick: Duration,
    hold_ms: u64,
    current: usize,
    next: usize,
    direction: i8,
    reduced_motion: bool,
    selector: PoseSelector,
    hold_random: Random,
}

impl PaperFlip {
    /// Start with pose one, independently seeded hold jitter, and one queued weighted pose.
    pub fn new(seed: u64) -> Self {
        let mut selector = PoseSelector::new(seed);
        selector.commit(0);
        let next = selector.choose_next(0);
        let mut hold_random = Random(seed ^ 0xA076_1D64_78BD_642F);
        let hold_ms = sample_hold(&mut hold_random);
        Self {
            phase: FlipPhase::Hold,
            phase_started: Duration::ZERO,
            last_tick: Duration::ZERO,
            hold_ms,
            current: 0,
            next,
            direction: 1,
            reduced_motion: false,
            selector,
            hold_random,
        }
    }

    /// Advance one timing phase using monotonic elapsed time since this animation began.
    pub fn tick(&mut self, elapsed: Duration) {
        let elapsed = elapsed.max(self.last_tick);
        self.last_tick = elapsed;
        if self.reduced_motion {
            return;
        }
        let phase_elapsed = elapsed.saturating_sub(self.phase_started);
        match self.phase {
            FlipPhase::Hold if phase_elapsed >= Duration::from_millis(self.hold_ms) => {
                self.start_flip(elapsed);
            }
            FlipPhase::Flip if phase_elapsed >= Duration::from_millis(FLIP_MS) => {
                self.current = self.next;
                self.selector.commit(self.current);
                self.next = self.selector.choose_next(self.current);
                self.phase = FlipPhase::Hold;
                self.phase_started = elapsed;
                self.hold_ms = sample_hold(&mut self.hold_random);
            }
            _ => {}
        }
    }

    /// Request an immediate flip during hold; ignore repeated clicks and reduced motion.
    pub fn trigger_flip(&mut self, elapsed: Duration) -> bool {
        if self.phase != FlipPhase::Hold || self.reduced_motion {
            return false;
        }
        let elapsed = elapsed.max(self.last_tick);
        self.last_tick = elapsed;
        self.start_flip(elapsed);
        true
    }

    /// Freeze the committed pose and restart a full hold when motion is enabled again.
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        if enabled != self.reduced_motion {
            self.reduced_motion = enabled;
            self.phase = FlipPhase::Hold;
            self.phase_started = self.last_tick;
        }
    }

    /// Return the frame for an elapsed time without mutating timing or selection.
    pub fn frame(&self, elapsed: Duration) -> AnimationFrame {
        let progress = if self.phase == FlipPhase::Flip && !self.reduced_motion {
            (elapsed.saturating_sub(self.phase_started).as_secs_f64() / 0.390).clamp(0.0, 1.0)
        } else {
            0.0
        };
        AnimationFrame {
            pose: if progress >= 0.5 { self.next } else { self.current },
            progress,
            direction: self.direction,
        }
    }

    /// Report the active timing phase for scheduling and qualification assertions.
    pub const fn phase(&self) -> FlipPhase {
        self.phase
    }

    /// Report the sampled hold, bounded to the source's inclusive 800–1100 milliseconds.
    pub const fn hold_duration(&self) -> Duration {
        Duration::from_millis(self.hold_ms)
    }

    /// Enter a new flip without moving the queued pose's ownership prematurely.
    fn start_flip(&mut self, elapsed: Duration) {
        self.phase = FlipPhase::Flip;
        self.phase_started = elapsed;
        self.direction *= -1;
    }
}

/// Keep selection probabilities and five committed recency entries with their sole owner.
struct PoseSelector {
    random: Random,
    recent: [Option<usize>; 5],
}

impl PoseSelector {
    /// Begin a deterministic sequence without any committed poses.
    fn new(seed: u64) -> Self {
        Self { random: Random(seed), recent: [None; 5] }
    }

    /// Suppress the most recently committed poses using the source multipliers.
    fn weight(&self, pose: usize) -> f64 {
        let base = match pose {
            0..28 => 1.0,
            35 => 0.125,
            _ => 0.25,
        };
        let multiplier = self
            .recent
            .iter()
            .position(|recent| *recent == Some(pose))
            .map_or(1.0, |index| RECENCY_MULTIPLIERS[index]);
        base * multiplier
    }

    /// Choose a weighted candidate while excluding the currently committed pose.
    fn choose_next(&mut self, current: usize) -> usize {
        let total: f64 =
            (0..POSE_COUNT).filter(|pose| *pose != current).map(|pose| self.weight(pose)).sum();
        let mut remainder = self.random.unit() * total;
        let mut last = 0;
        for pose in (0..POSE_COUNT).filter(|pose| *pose != current) {
            last = pose;
            remainder -= self.weight(pose);
            if remainder < 0.0 {
                return pose;
            }
        }
        last
    }

    /// Commit only visible poses, with the newest entry receiving the strongest penalty.
    fn commit(&mut self, pose: usize) {
        self.recent.rotate_right(1);
        self.recent[0] = Some(pose);
    }
}

/// Supply portable deterministic random values without a runtime dependency or global state.
struct Random(u64);

impl Random {
    /// Generate one SplitMix64 value using defined wrapping integer arithmetic.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// Draw a value in the half-open zero-through-one interval.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}

/// Sample the source's 950 ± 150 millisecond inclusive jitter without modulo bias.
fn sample_hold(random: &mut Random) -> u64 {
    let limit = u64::MAX - u64::MAX % 301;
    loop {
        let value = random.next();
        if value < limit {
            return 800 + value % 301;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FlipPhase, PaperFlip, PoseSelector};
    use std::time::Duration;

    /// Match source phase transitions, midpoint image selection, and click suppression.
    #[test]
    fn click_turns_committed_pose_into_queued_pose_once() {
        let mut animation = PaperFlip::new(7);
        assert!(animation.trigger_flip(Duration::from_millis(100)));
        assert!(!animation.trigger_flip(Duration::from_millis(101)));
        let early = animation.frame(Duration::from_millis(294));
        let middle = animation.frame(Duration::from_millis(295));
        assert_eq!(early.pose, 0);
        assert_ne!(middle.pose, 0);
        assert_eq!(middle.progress, 0.5);
        assert_eq!(middle.direction, -1);
        animation.tick(Duration::from_millis(490));
        assert_eq!(animation.phase(), FlipPhase::Hold);
        assert_eq!(animation.frame(Duration::from_millis(490)).pose, middle.pose);
        assert!(animation.trigger_flip(Duration::from_millis(500)));
        assert_eq!(animation.frame(Duration::from_millis(500)).direction, 1);
    }

    /// Preserve a bounded hold and deterministic seeded selection across repeated cycles.
    #[test]
    fn seeded_sequences_are_repeatable_and_never_repeat_current() {
        let mut first = PaperFlip::new(41);
        let mut second = PaperFlip::new(41);
        let mut now = Duration::ZERO;
        for _ in 0..200 {
            assert_eq!(first.hold_duration(), second.hold_duration());
            assert!(
                (Duration::from_millis(800)..=Duration::from_millis(1100))
                    .contains(&first.hold_duration())
            );
            let previous = first.frame(now).pose;
            now += first.hold_duration();
            first.tick(now);
            second.tick(now);
            assert_eq!(first.phase(), FlipPhase::Flip);
            now += Duration::from_millis(390);
            first.tick(now);
            second.tick(now);
            assert_eq!(first.frame(now), second.frame(now));
            assert_ne!(first.frame(now).pose, previous);
        }
    }

    /// Retain a static committed pose while reduced motion is active.
    #[test]
    fn reduced_motion_does_not_consume_pose_history() {
        let mut animation = PaperFlip::new(19);
        animation.set_reduced_motion(true);
        animation.tick(Duration::from_secs(10));
        assert_eq!(animation.frame(Duration::from_secs(10)).pose, 0);
        assert!(!animation.trigger_flip(Duration::from_secs(10)));
        animation.set_reduced_motion(false);
        animation.tick(Duration::from_secs(10));
        assert_eq!(animation.phase(), FlipPhase::Hold);
        animation.tick(Duration::from_secs(10) + animation.hold_duration());
        assert_eq!(animation.phase(), FlipPhase::Flip);
    }

    /// Characterize the source's rarity and newest-first recency penalties.
    #[test]
    fn selector_preserves_rarity_and_recency() {
        let mut selector = PoseSelector::new(0);
        assert_eq!(selector.weight(0), 1.0);
        assert_eq!(selector.weight(28), 0.25);
        assert_eq!(selector.weight(35), 0.125);
        selector.commit(0);
        assert_eq!(selector.weight(0), 0.10);
        selector.commit(1);
        assert_eq!(selector.weight(0), 0.20);
        selector.commit(2);
        selector.commit(3);
        selector.commit(4);
        assert_eq!(selector.weight(0), 0.75);
        selector.commit(5);
        assert_eq!(selector.weight(0), 1.0);
    }

    /// Avoid catch-up loops and unsigned underflow when the injected clock stalls or regresses.
    #[test]
    fn delayed_and_regressing_ticks_remain_safe() {
        let mut animation = PaperFlip::new(3);
        animation.tick(Duration::from_secs(100));
        assert_eq!(animation.phase(), FlipPhase::Flip);
        assert_eq!(animation.frame(Duration::from_secs(100)).progress, 0.0);
        animation.tick(Duration::ZERO);
        assert_eq!(animation.phase(), FlipPhase::Flip);
        animation.tick(Duration::from_secs(101));
        assert_eq!(animation.phase(), FlipPhase::Hold);
    }
}
