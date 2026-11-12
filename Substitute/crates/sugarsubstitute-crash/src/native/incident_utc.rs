// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample precise UTC only inside the genuine finite original committed-reader operation.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::AdmissionError,
    incident::{IncidentError, IncidentUtc},
};
use sugarsubstitute_execution::CommittedReaderOrigin;
use windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
/// An explicit wallclock adapter injected by the finite reporting helper.
pub struct SystemIncidentUtc;
/// Preserve original Core controls or checked UTC conversion rejection.
pub enum IncidentUtcReadError {
    /// Genuine original admission/cancellation/deadline/clock failure.
    Control(AdmissionError),
    /// Native wallclock value cannot be represented by the domain's UTC range.
    Time(IncidentError),
}
impl fmt::Debug for IncidentUtcReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IncidentUtcReadError")
    }
}
impl fmt::Display for IncidentUtcReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Incident UTC observation rejected")
    }
}
impl Error for IncidentUtcReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Control(cause) => cause,
            Self::Time(cause) => cause,
        })
    }
}
impl SystemIncidentUtc {
    /// Sample UTC whole seconds with actual original Core controls before and after.
    /// No deadline, scope or operation is renewed by wallclock sampling.
    /// # Errors
    /// Preserves genuine original control failures and rejects pre-Unix/year10000 values.
    pub fn sample(
        &self,
        original: &CommittedReaderOrigin,
    ) -> Result<IncidentUtc, IncidentUtcReadError> {
        original.check_effect().map_err(IncidentUtcReadError::Control)?;
        // Safety: parameter-free native time observation returns a FILETIME by value.
        // It retains no callbacks, memory, handles or borrowed state.
        let time = unsafe { GetSystemTimePreciseAsFileTime() };
        original.check_effect().map_err(IncidentUtcReadError::Control)?;
        filetime_seconds((u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime))
            .map_err(IncidentUtcReadError::Time)
    }
}
fn filetime_seconds(ticks: u64) -> Result<IncidentUtc, IncidentError> {
    let seconds =
        ticks.checked_sub(116_444_736_000_000_000).ok_or(IncidentError::InvalidTime)? / 10_000_000;
    IncidentUtc::from_unix_seconds(i64::try_from(seconds).map_err(|_| IncidentError::InvalidTime)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filetime_conversion_preserves_unix_whole_seconds_and_fractional_floor()
    -> Result<(), IncidentError> {
        assert_eq!(filetime_seconds(116_444_736_000_000_000)?.unix_seconds(), 0);
        assert_eq!(filetime_seconds(116_444_736_019_999_999)?.unix_seconds(), 1);
        assert!(filetime_seconds(116_444_735_999_999_999).is_err());
        assert!(filetime_seconds(116_444_736_000_000_000 + 253_402_300_800 * 10_000_000).is_err());
        Ok(())
    }
}
