// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The preparation helper's invocation tail and exit classes. The helper reports what its
//! attempt did over the inherited update-attempt channel; the exit code is only a coarse class.

/// Introduces the inherited update-attempt channel: the bootstrap section token, then the
/// original handshake deadline's counter ticks and frequency.
pub const ATTEMPT_ARGUMENT: &str = "--update-attempt";

/// Coarse class of a preparation helper's exit. What the attempt did travels on the channel;
/// a code never distinguishes "prepared" from "up to date".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperExit {
    /// The helper reached its end and retired everything it owned.
    Retired,
    /// The helper stopped on an error it could not report as its result.
    Failed,
}
impl HelperExit {
    /// Process exit code of this class.
    pub const fn code(self) -> u32 {
        match self {
            Self::Retired => 0,
            Self::Failed => 1,
        }
    }

    /// Class of an observed exit code; any code other than `Retired`'s is a failure.
    pub const fn from_code(code: u32) -> Self {
        if code == 0 { Self::Retired } else { Self::Failed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_classes_round_trip_and_unknown_codes_are_failures() {
        for class in [HelperExit::Retired, HelperExit::Failed] {
            assert_eq!(HelperExit::from_code(class.code()), class);
        }
        for code in [2, 101, 0xc000_0005] {
            assert_eq!(HelperExit::from_code(code), HelperExit::Failed);
        }
    }
}
