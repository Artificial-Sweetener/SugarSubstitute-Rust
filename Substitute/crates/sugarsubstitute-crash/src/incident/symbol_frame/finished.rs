// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the actual still-held finished token before publication, without a second dump copy.
use super::*;
use crate::incident::CapturedReport;
use sugarsubstitute_contracts::execution::JobContext;
struct Finished<'a> {
    report: &'a mut CapturedReport,
    context: &'a JobContext,
}
impl Read for Finished<'_> {
    fn bytes(&mut self, offset: u64, output: &mut [u8]) -> Result<(), SymbolFrameError> {
        let token = self.report.token.as_mut().ok_or(SymbolFrameError::Corrupt)?;
        let count = token
            .read(offset, output, self.context)
            .map_err(|_| SymbolFrameError::Source(DumpReadCode::Native))?;
        if count != output.len() {
            return Err(SymbolFrameError::Corrupt);
        }
        Ok(())
    }
}
impl CapturedReport {
    /// Extract native exception/code/module facts from this exact already-validated finished source.
    /// The native token has independently validated subject/build/UUID and keeps the source inode.
    /// No module path, handle, new file read authority or copied dump escapes this operation.
    /// # Errors
    /// A partial native/control failure remains with failure()/clock_failure()/panic_payload().
    /// Malformed exception/module/CodeView bytes never manufacture a recovered failure summary.
    pub fn exception_frame(
        &mut self,
        context: &JobContext,
    ) -> Result<CapturedExceptionFrame, SymbolFrameError> {
        let (_, length) = self.dump().ok_or(SymbolFrameError::Corrupt)?;
        // C++ validated the annotation carrier; it does not export that module's index.
        // Actual exception-containing module selection remains unique and independently bounded.
        extract(&mut Finished { report: self, context }, length, None, FailureEvidence::Unexplained)
    }
}
