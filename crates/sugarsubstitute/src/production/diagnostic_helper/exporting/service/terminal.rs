// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer unchanged authenticated completion loans while retaining the exact controller source.
use super::*;
/// Original received/issued loans and the complete source stay together through router disposal.
pub(in crate::production) struct ExportTerminal {
    pub(in crate::production) received: ReceivedDiagnosticRequest,
    pub(in crate::production) issued: IssuedDiagnosticRequest,
    pub(in crate::production) receipt: Option<ReportExportReceipt>,
    pub(in crate::production) view: Option<PreparedReportView>,
    pub(super) _source: Source,
}

/// Nonclone metadata lifetime retained through actual finite Core router acceptance.
pub(in crate::production) struct ExportCompletionCustody {
    _source: Source,
}
/// Transfer the original authenticated loans, actual result and complete charged view.
pub(in crate::production) type ExportCompletionParts = (
    ReceivedDiagnosticRequest,
    IssuedDiagnosticRequest,
    Option<ReportExportReceipt>,
    Option<PreparedReportView>,
    ExportCompletionCustody,
);
impl ExportTerminal {
    /// Move the same source guard alongside the original tokens into the actual Core completion.
    /// The caller retains custody and the whole view until matched accepted router delivery.
    pub(in crate::production) fn into_completion_parts(self) -> ExportCompletionParts {
        (
            self.received,
            self.issued,
            self.receipt,
            self.view,
            ExportCompletionCustody { _source: self._source },
        )
    }
}
