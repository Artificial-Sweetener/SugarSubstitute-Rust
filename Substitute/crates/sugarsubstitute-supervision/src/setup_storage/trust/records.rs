// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read bounded protected records through their exact installation/repository envelope.
use super::*;
impl TrustStore {
    /// Verify exact domain/binding and native protection before interpreting any local bytes.
    pub(super) fn read_bound_record(
        &self,
        name: &str,
        kind: envelope::Kind,
        protection: &Protection,
    ) -> Result<Option<Vec<u8>>, TrustFailure> {
        let mut file = match self.root.read(name) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause.into()),
        };
        protection.verify_file(&file)?;
        let bytes = files::read_bounded(&mut file, kind.limit())?;
        Ok(Some(envelope::decode(kind, &self.installation, &self.repository, &bytes)?.to_vec()))
    }

    /// Publish a complete bound record under the caller's retained trust writer.
    pub(super) fn replace_bound_record(
        &self,
        name: &str,
        kind: envelope::Kind,
        payload: &[u8],
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let bytes = envelope::encode(kind, &self.installation, &self.repository, payload)?;
        let staged = staging::unique_path(&self.root, "record-pending")?;
        let mut file = protection.file(&staged, context)?;
        file.write_all(&bytes)?;
        files::sync(&file, context)?;
        drop(file);
        check(context)?;
        staging::replace_record(&staged, &self.root.path().join(name))?;
        Ok(())
    }

    /// Retain immutable evidence before a selection can reference it; interrupted siblings are inert.
    pub(super) fn retain_bound_record(
        &self,
        name: &str,
        kind: envelope::Kind,
        payload: &[u8],
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let bytes = envelope::encode(kind, &self.installation, &self.repository, payload)?;
        files::write_immutable(&self.root.path().join(name), &bytes, |path| {
            protection.file(path, context)
        })?;
        protection.verify_file(&self.root.read(name)?)?;
        Ok(())
    }
}
