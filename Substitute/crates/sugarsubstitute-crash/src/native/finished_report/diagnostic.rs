// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only closed retained native/control categories; never invoke a cause formatter.
use super::{
    control::ControlCause,
    observation::{Failure, Origin},
};
use sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageFailureCode;

/// Closed origin reported by the authenticated finished-report native boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishedFailureOrigin {
    /// Native operating-system operation.
    Native,
    /// Original execution control observation.
    Control,
    /// Native counter observation.
    Qpc,
}
/// Closed leaf category, independent of error ownership and native disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishedFailureCategory {
    /// The native result retained its actual origin and numeric code.
    Native,
    /// Original cancellation was observed.
    Cancelled,
    /// The original deadline elapsed.
    TimedOut,
    /// Required original deadline was missing.
    MissingDeadline,
    /// A different clock owner was supplied.
    ForeignClock,
    /// The original clock provider failed.
    Clock,
    /// The exact protected source guard rejected an operation.
    Source,
    /// A provider unwound; its opaque payload stays private.
    Panicked,
    /// Native output failed strict validation.
    InvalidResult,
    /// A retained settlement operation rejected its typed phase or identity.
    Rejected,
}
/// Bounded passive leaf data; it grants no capture, file or cleanup authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinishedFailureDiagnostic {
    /// Original retained leaf category.
    pub category: FinishedFailureCategory,
    /// Actual native origin, only when the retained cause contains one.
    pub native_origin: Option<FinishedFailureOrigin>,
    /// Exact numeric native code; no system or provider message is formatted.
    pub native_code: Option<u32>,
    /// Exact protected-source rejection, when the original control retained it.
    pub storage_code: Option<DiagnosticStorageFailureCode>,
}
impl FinishedFailureDiagnostic {
    pub(super) const fn category(category: FinishedFailureCategory) -> Self {
        Self { category, native_origin: None, native_code: None, storage_code: None }
    }
    pub(super) const fn native(origin: Origin, code: u32) -> Self {
        Self {
            category: FinishedFailureCategory::Native,
            native_origin: Some(match origin {
                Origin::Native => FinishedFailureOrigin::Native,
                Origin::Control => FinishedFailureOrigin::Control,
                Origin::Qpc => FinishedFailureOrigin::Qpc,
            }),
            native_code: Some(code),
            storage_code: None,
        }
    }
    pub(super) const fn source(code: DiagnosticStorageFailureCode) -> Self {
        Self { storage_code: Some(code), ..Self::category(FinishedFailureCategory::Source) }
    }
}
impl Failure {
    pub(super) fn diagnostic(&self) -> FinishedFailureDiagnostic {
        use FinishedFailureCategory as Code;
        match self {
            Self::Native { origin, code } => FinishedFailureDiagnostic::native(*origin, *code),
            Self::Invalid(_) => FinishedFailureDiagnostic::category(Code::InvalidResult),
            Self::Control(cause) => match cause {
                ControlCause::Cancelled => FinishedFailureDiagnostic::category(Code::Cancelled),
                ControlCause::TimedOut => FinishedFailureDiagnostic::category(Code::TimedOut),
                ControlCause::MissingDeadline => {
                    FinishedFailureDiagnostic::category(Code::MissingDeadline)
                }
                ControlCause::ForeignClock => {
                    FinishedFailureDiagnostic::category(Code::ForeignClock)
                }
                ControlCause::Clock(_) => FinishedFailureDiagnostic::category(Code::Clock),
                ControlCause::Source(cause) => FinishedFailureDiagnostic::source(cause.code()),
                ControlCause::Panic(_) => FinishedFailureDiagnostic::category(Code::Panicked),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_finished_native_projection_preserves_exact_origin_and_code() {
        for (origin, expected) in [
            (Origin::Native, FinishedFailureOrigin::Native),
            (Origin::Control, FinishedFailureOrigin::Control),
            (Origin::Qpc, FinishedFailureOrigin::Qpc),
        ] {
            let cause = Failure::Native { origin, code: 0x80000003 };
            let diagnostic = cause.diagnostic();
            assert_eq!(diagnostic.category, FinishedFailureCategory::Native);
            assert_eq!(diagnostic.native_origin, Some(expected));
            assert_eq!(diagnostic.native_code, Some(0x80000003));
            assert!(diagnostic.storage_code.is_none());
        }
    }
    #[test]
    fn retained_finished_control_projection_keeps_opaque_payload_private_and_borrowed() {
        let cases = [
            (ControlCause::Cancelled, FinishedFailureCategory::Cancelled),
            (ControlCause::TimedOut, FinishedFailureCategory::TimedOut),
            (ControlCause::MissingDeadline, FinishedFailureCategory::MissingDeadline),
            (ControlCause::ForeignClock, FinishedFailureCategory::ForeignClock),
            (
                ControlCause::Panic(Box::new("private-provider-payload")),
                FinishedFailureCategory::Panicked,
            ),
        ];
        for (cause, expected) in cases {
            let cause = Failure::Control(cause);
            let original = std::ptr::from_ref(&cause);
            assert_eq!(cause.diagnostic().category, expected);
            assert!(!format!("{:?}", cause.diagnostic()).contains("private-provider"));
            assert_eq!(std::ptr::from_ref(&cause), original);
            if expected == FinishedFailureCategory::Panicked {
                assert!(matches!(
                    &cause,
                    Failure::Control(ControlCause::Panic(payload))
                        if payload.downcast_ref::<&str>() == Some(&"private-provider-payload")
                ));
            }
        }
    }
}
