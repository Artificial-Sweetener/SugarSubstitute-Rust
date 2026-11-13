// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the time that dates strikes and decides whether they still withhold a package.
use super::SelectionFailure;

/// The UTC Unix seconds a selection store dates strikes with and checks withholding against.
///
/// Every record transition that stamps time (a rejected trial, an abandoned trial that counts as
/// a strike, a settled bad verdict) and every withholding decision reads this one clock, so a
/// store under test can be given a fixed time. Strikes only expire, so a wrong system clock at worst gives
/// a release one more trial or withholds it for at most one strike lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum StrikeClock {
    /// The operating system's wall clock, read at each transition.
    #[default]
    System,
    /// One fixed instant, in UTC Unix seconds.
    #[cfg(test)]
    Fixed(u64),
}

impl StrikeClock {
    /// The current time in UTC Unix seconds.
    /// # Errors
    /// Rejects a system clock set before the Unix epoch.
    pub(crate) fn now(self) -> Result<u64, SelectionFailure> {
        match self {
            Self::System => {
                let elapsed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)?;
                Ok(elapsed.as_secs())
            }
            #[cfg(test)]
            Self::Fixed(at) => Ok(at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed clock answers exactly its instant, however often it is read.
    #[test]
    fn a_fixed_clock_answers_its_instant() -> Result<(), SelectionFailure> {
        let clock = StrikeClock::Fixed(1_700_000_000);
        assert_eq!(clock.now()?, 1_700_000_000);
        assert_eq!(clock.now()?, 1_700_000_000);
        assert_eq!(StrikeClock::default(), StrikeClock::System);
        Ok(())
    }
}
