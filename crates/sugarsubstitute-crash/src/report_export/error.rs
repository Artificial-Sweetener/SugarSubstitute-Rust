// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original export failures separate from inert action-result categories.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::AdmissionError;
/// Bounded export outcome, without native paths or sensitive dump contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportExportCode {
    /// Current incident/revision/session or immutable payload disagrees.
    Identity,
    /// Selected Windows destination exceeds the closed lexical contract.
    Destination,
    /// Destination exists and overwrite was not explicitly selected.
    Consent,
    /// Genuine finite execution controls rejected the operation.
    Control,
    /// Selected destination could not be opened, written, flushed or activated.
    Native,
    /// Genuine committed reader rejected the selected native dump.
    Dump,
    /// Checked retirement failed; physical outputs remain retained.
    Retirement,
    /// Exact complete bounded transfer was not observed.
    Bounds,
}
/// Original cause retained with receiving outputs and original reader custody.
#[derive(Debug)]
pub enum ReportExportCause {
    /// Closed validation failure, with no native permission implied.
    Rejected(ReportExportCode),
    /// Actual central execution clock, cancellation, deadline or admission cause.
    Control(AdmissionError),
    /// Actual OS failure from the selected receiving-file owner.
    Native(windows::core::Error),
    /// Categorical read failure; original native cause remains in the same dump lease.
    Dump(crate::incident::DumpReadCode),
}
impl From<sugarsubstitute_supervision::ReportFileError> for ReportExportCause {
    fn from(cause: sugarsubstitute_supervision::ReportFileError) -> Self {
        use sugarsubstitute_supervision::{ReportFileCode as Code, ReportFileError as Cause};
        match cause {
            Cause::Rejected(code) => Self::Rejected(match code {
                Code::Identity => ReportExportCode::Identity,
                Code::Destination => ReportExportCode::Destination,
                Code::Consent => ReportExportCode::Consent,
                Code::Bounds => ReportExportCode::Bounds,
            }),
            Cause::Control(cause) => Self::Control(cause),
            Cause::Native(cause) => Self::Native(cause),
        }
    }
}
impl ReportExportCause {
    /// Project a closed result category while preserving this original cause.
    pub(super) fn code(&self) -> ReportExportCode {
        match self {
            Self::Rejected(code) => *code,
            Self::Control(_) => ReportExportCode::Control,
            Self::Native(_) => ReportExportCode::Native,
            Self::Dump(_) => ReportExportCode::Dump,
        }
    }
}
impl fmt::Display for ReportExportCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Report export rejected: {:?}", self.code())
    }
}
impl Error for ReportExportCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_supervision::{ReportFileCode as Code, ReportFileError as Cause};

    #[test]
    fn shared_storage_rejections_preserve_report_contract_categories() {
        for (storage, report) in [
            (Code::Identity, ReportExportCode::Identity),
            (Code::Destination, ReportExportCode::Destination),
            (Code::Consent, ReportExportCode::Consent),
            (Code::Bounds, ReportExportCode::Bounds),
        ] {
            assert_eq!(ReportExportCause::from(Cause::Rejected(storage)).code(), report);
        }
    }

    #[test]
    fn shared_storage_failure_preserves_original_native_cause()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = windows::Win32::Foundation::ERROR_SHARING_VIOLATION.to_hresult();
        let cause =
            ReportExportCause::from(Cause::Native(windows::core::Error::from_hresult(code)));
        assert_eq!(cause.code(), ReportExportCode::Native);
        let original = cause
            .source()
            .and_then(|source| source.downcast_ref::<windows::core::Error>())
            .ok_or("original storage cause absent")?;
        assert_eq!(original.code(), code);
        Ok(())
    }

    #[test]
    fn shared_storage_failure_preserves_original_execution_cause()
    -> Result<(), Box<dyn std::error::Error>> {
        let cause = ReportExportCause::from(Cause::Control(AdmissionError::ShuttingDown));
        assert_eq!(cause.code(), ReportExportCode::Control);
        let original = cause
            .source()
            .and_then(|source| source.downcast_ref::<AdmissionError>())
            .ok_or("original execution cause absent")?;
        assert!(matches!(original, AdmissionError::ShuttingDown));
        Ok(())
    }
}
