// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close only actual owned outputs under the original independently activated cleanup.
use super::*;
use sugarsubstitute_execution::CommittedReaderRetirement;
impl CommittedDiagnosticReader {
    /// Consume the complete transaction after checked data/fence/token/directory closure.
    /// # Errors
    /// Every fresh close checks the actual same-origin proof. Accepted late returns
    /// adopt only their own output facts; uncertainty returns identical whole custody.
    pub fn retire(
        mut self,
        proof: &CommittedReaderRetirement,
    ) -> Result<(), CommittedReaderRetirementFailure> {
        let result = self.retire_inner(proof);
        match result {
            Ok(()) => Ok(()),
            Err(cause) => Err(CommittedReaderRetirementFailure { owner: self, cause }),
        }
    }
    fn retire_inner(&mut self, proof: &CommittedReaderRetirement) -> Result<(), ClaimError> {
        let (fields, origin) = self.parts()?;
        let proof = crate::installed_release::SelectedRetirement::CommittedReader(proof);
        if !proof.matches_origin(origin.original()) {
            return Err(missing());
        }
        origin.close().map_err(ClaimError::capture_allocation)?;
        for file in fields.files.iter_mut().flatten() {
            file.retire_selected(&proof)?;
        }
        if let Some(writer) = fields.writer.as_mut() {
            writer.retire_reader(&proof)?;
        }
        for index in (0..fields.directories.len()).rev() {
            fields.directories[index]
                .close_selected(&proof, &mut fields.directory_causes[index])?;
        }
        fields.token.close_selected(&proof, &mut fields.token_causes)?;
        Ok(())
    }
}
