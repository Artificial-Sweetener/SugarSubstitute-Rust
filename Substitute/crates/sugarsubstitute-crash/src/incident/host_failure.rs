// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded host operation facts independently of native crash capture.
use super::IncidentStoreError;
use std::fmt;
use sugarsubstitute_contracts::{diagnostics::DiagnosticOperation, incident::IncidentError};

/// A recorded application error category, independent of the process termination mechanism.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostFailureCategory {
    /// A direct native HRESULT.
    Hresult,
    /// A direct operating-system code.
    Os,
    /// A categorical I/O failure.
    Io,
    /// A categorical frontend-readiness failure.
    Readiness,
    /// JSON input/output failed.
    JsonIo,
    /// JSON syntax was invalid.
    JsonSyntax,
    /// JSON data did not satisfy its contract.
    JsonData,
    /// JSON input ended prematurely.
    JsonEof,
    /// The owner retained an error without a publishable category.
    Opaque,
}
impl HostFailureCategory {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "Hresult" => Self::Hresult,
            "Os" => Self::Os,
            "Io" => Self::Io,
            "Readiness" => Self::Readiness,
            "JsonIo" => Self::JsonIo,
            "JsonSyntax" => Self::JsonSyntax,
            "JsonData" => Self::JsonData,
            "JsonEof" => Self::JsonEof,
            "Opaque" => Self::Opaque,
            _ => return None,
        })
    }
}

/// Validated identifiers and numeric facts; raw errors, arguments and paths cannot enter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostFailureFact {
    operation: String,
    observation: String,
    category: HostFailureCategory,
    detail: Option<String>,
    native_code: Option<i32>,
    launch: u64,
    navigation: u64,
}
impl HostFailureFact {
    /// Maximum encoded bytes, including schema marker and final newline.
    pub const MAX_BYTES: usize = 1024;
    /// Maximum heap bytes of one retained fact, independently of its inline metadata.
    pub const MAX_HEAP_BYTES: usize =
        3 * sugarsubstitute_contracts::diagnostics::DIAGNOSTIC_OPERATION_BYTES;

    /// Copy identifiers already selected by the authoritative host error owner.
    /// This grants no native or filesystem authority and does not classify termination.
    /// # Errors
    /// Rejects free-form text, zero identities, inconsistent categories and allocation failure.
    pub fn new(
        operation: &str,
        observation: &str,
        category: HostFailureCategory,
        detail: Option<&str>,
        native_code: Option<i32>,
        launch: u64,
        navigation: u64,
    ) -> Result<Self, IncidentStoreError> {
        let numeric = matches!(category, HostFailureCategory::Hresult | HostFailureCategory::Os);
        let named = matches!(category, HostFailureCategory::Io | HostFailureCategory::Readiness);
        if launch == 0
            || navigation == 0
            || numeric != native_code.is_some()
            || named != detail.is_some()
            || detail == Some("None")
        {
            return Err(IncidentError::InvalidEvidence.into());
        }
        Ok(Self {
            operation: label(operation)?,
            observation: label(observation)?,
            category,
            detail: detail.map(label).transpose()?,
            native_code,
            launch,
            navigation,
        })
    }

    /// Decode only the canonical version-one host-failure appendix.
    /// # Errors
    /// Rejects unknown schemas, extra fields, noncanonical numbers and free-form text.
    pub fn decode(bytes: &[u8]) -> Result<Self, IncidentStoreError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(IncidentStoreError::RecordBound);
        }
        let invalid = || IncidentStoreError::Facts(IncidentError::InvalidEvidence);
        let line = std::str::from_utf8(bytes).map_err(|_| invalid())?;
        let mut fields = line.strip_suffix('\n').ok_or_else(invalid)?.split(' ');
        if fields.next() != Some("host_failure_v1") {
            return Err(invalid());
        }
        let operation =
            fields.next().and_then(|s| s.strip_prefix("operation=")).ok_or_else(invalid)?;
        let observation =
            fields.next().and_then(|s| s.strip_prefix("observation=")).ok_or_else(invalid)?;
        let category = fields
            .next()
            .and_then(|s| s.strip_prefix("category="))
            .and_then(HostFailureCategory::parse)
            .ok_or_else(invalid)?;
        let detail = fields.next().and_then(|s| s.strip_prefix("detail=")).ok_or_else(invalid)?;
        let code =
            fields.next().and_then(|s| s.strip_prefix("native_code=")).ok_or_else(invalid)?;
        let launch = fields
            .next()
            .and_then(|s| s.strip_prefix("launch="))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(invalid)?;
        let navigation = fields
            .next()
            .and_then(|s| s.strip_prefix("navigation="))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(invalid)?;
        if fields.next().is_some() {
            return Err(invalid());
        }
        let fact = Self::new(
            operation,
            observation,
            category,
            (detail != "None").then_some(detail),
            if code == "None" { None } else { Some(code.parse().map_err(|_| invalid())?) },
            launch,
            navigation,
        )?;
        if fact.to_string().as_bytes() != bytes {
            return Err(invalid());
        }
        Ok(fact)
    }
}
fn label(value: &str) -> Result<String, IncidentStoreError> {
    DiagnosticOperation::new(value).map_err(|_| IncidentError::InvalidEvidence)?;
    let mut result = String::new();
    result.try_reserve_exact(value.len()).map_err(|_| IncidentStoreError::Capacity)?;
    result.push_str(value);
    Ok(result)
}
impl fmt::Display for HostFailureFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host_failure_v1 operation={} observation={} category={:?} detail={} native_code=",
            self.operation,
            self.observation,
            self.category,
            self.detail.as_deref().unwrap_or("None")
        )?;
        match self.native_code {
            Some(value) => write!(f, "{value}")?,
            None => f.write_str("None")?,
        }
        writeln!(f, " launch={} navigation={}", self.launch, self.navigation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_failure_roundtrip_preserves_actionable_cause_without_native_capture()
    -> Result<(), IncidentStoreError> {
        let fact = HostFailureFact::new(
            "main_frontend_effect",
            "None",
            HostFailureCategory::Io,
            Some("Io"),
            None,
            1,
            2,
        )?;
        assert_eq!(HostFailureFact::decode(fact.to_string().as_bytes())?, fact);
        for bad in ["C:\\private", "password=value", "Bearer secret", "operation\nlaunch=3"] {
            assert!(
                HostFailureFact::new(bad, "None", HostFailureCategory::Opaque, None, None, 1, 2)
                    .is_err()
            );
        }
        for bad in [
            fact.to_string().replace("launch=1", "launch=01"),
            fact.to_string().replace("category=Io", "category=Private"),
            format!("{} secret=value\n", fact.to_string().trim_end()),
        ] {
            assert!(HostFailureFact::decode(bad.as_bytes()).is_err());
        }
        Ok(())
    }
}
