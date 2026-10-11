// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Represent execution ownership and monotonic time without scheduler handles.

use std::{error::Error, fmt, num::NonZeroU64, time::Duration};

/// Reject zero identities and exhausted generation sequences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// Zero cannot identify an owner or revision.
    Zero,
    /// The positive identity range has been exhausted.
    Exhausted,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Zero => "Execution identity must be positive",
            Self::Exhausted => "Execution identity range exhausted",
        })
    }
}
impl Error for IdentityError {}

macro_rules! identity {
    ($name:ident, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);

        impl $name {
            /// Construct a positive identity.
            ///
            /// # Errors
            /// Returns `IdentityError::Zero` when the value is zero.
            pub const fn new(value: u64) -> Result<Self, IdentityError> {
                match NonZeroU64::new(value) {
                    Some(value) => Ok(Self(value)),
                    None => Err(IdentityError::Zero),
                }
            }

            /// Return the positive identity value without changing its ownership.
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }
    };
}

identity!(ExecutionId, "Identify one adapter-owned executor lifetime.");
identity!(ScopeId, "Identify one application-owned scope within an executor lifetime.");
identity!(JobId, "Identify one application-owned job within an executor lifetime.");
identity!(SurfaceId, "Identify one application-owned presentation surface.");
identity!(Generation, "Identify one positive application-owned surface revision.");

impl Generation {
    /// Advance a surface revision without wrapping or reusing a generation.
    ///
    /// # Errors
    /// Returns `IdentityError::Exhausted` at the maximum generation.
    pub const fn next(self) -> Result<Self, IdentityError> {
        match self.get().checked_add(1) {
            Some(value) => Self::new(value),
            None => Err(IdentityError::Exhausted),
        }
    }
}

/// Reject overflowing or regressing monotonic nanosecond arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeError {
    /// A duration or resulting moment exceeds the `u64` nanosecond range.
    Overflow,
    /// A requested elapsed interval ends before its beginning.
    Regression,
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Overflow => "Monotonic nanosecond range exceeded",
            Self::Regression => "Monotonic interval regressed",
        })
    }
}
impl Error for TimeError {}

/// Nanoseconds since an execution clock's monotonic epoch; zero is valid.
///
/// Compare moments only within the same injected clock. This value carries no
/// wall-clock timestamp, native clock handle or process-global epoch assumption.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Moment(u64);

impl Moment {
    /// Construct a moment from monotonic nanoseconds in the owner's clock epoch.
    pub const fn from_nanos(nanoseconds: u64) -> Self {
        Self(nanoseconds)
    }

    /// Return the monotonic nanosecond offset within the owner's clock epoch.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Add an elapsed duration without truncation or wraparound.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` for an unrepresentable duration or result.
    pub fn checked_add(self, elapsed: Duration) -> Result<Self, TimeError> {
        let nanoseconds = u64::try_from(elapsed.as_nanos()).map_err(|_| TimeError::Overflow)?;
        self.0.checked_add(nanoseconds).map(Self).ok_or(TimeError::Overflow)
    }

    /// Measure elapsed time without silently accepting a regressing clock.
    ///
    /// # Errors
    /// Returns `TimeError::Regression` when `earlier` is after this moment.
    pub fn checked_duration_since(self, earlier: Self) -> Result<Duration, TimeError> {
        self.0.checked_sub(earlier.0).map(Duration::from_nanos).ok_or(TimeError::Regression)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_reject_zero_and_generations_never_wrap() -> Result<(), IdentityError> {
        assert_eq!(ScopeId::new(0), Err(IdentityError::Zero));
        assert_eq!(JobId::new(0), Err(IdentityError::Zero));
        assert_eq!(SurfaceId::new(0), Err(IdentityError::Zero));
        assert_eq!(ExecutionId::new(0), Err(IdentityError::Zero));
        assert_eq!(Generation::new(0), Err(IdentityError::Zero));
        assert_eq!(Generation::new(u64::MAX)?.next(), Err(IdentityError::Exhausted));
        assert_eq!(Generation::new(8)?.next()?.get(), 9);
        Ok(())
    }

    #[test]
    fn moments_preserve_nanoseconds_and_reject_invalid_arithmetic() -> Result<(), TimeError> {
        let zero = Moment::from_nanos(0);
        let later = zero.checked_add(Duration::new(3, 17))?;
        assert_eq!(later.as_nanos(), 3_000_000_017);
        assert_eq!(later.checked_duration_since(zero)?, Duration::new(3, 17));
        assert_eq!(zero.checked_duration_since(later), Err(TimeError::Regression));
        assert_eq!(zero.checked_add(Duration::MAX), Err(TimeError::Overflow));
        assert_eq!(
            Moment::from_nanos(u64::MAX).checked_add(Duration::from_nanos(1)),
            Err(TimeError::Overflow)
        );
        Ok(())
    }
}
