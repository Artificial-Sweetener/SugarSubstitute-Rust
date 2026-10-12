// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply fixed source-boundary facts through real admission and completion for adapter tests.
use super::{
    state::{Inspection, SourcePort},
    *,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
struct FixedSource;
impl SourcePort for FixedSource {
    fn inspect(
        &self,
        _: &FrontendInstallationInput,
        state: &mut Inspection,
        _: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        let mut manifest = FrontendSourceManifest {
            frontend: "1.47.11".into(),
            cube: "0.15.0".into(),
            assets: Vec::new(),
            cube_paths: Vec::new(),
            script: r"\\?\E:\fixture\main.py".into(),
            script_hash: Sha256::digest(b"fixture script").into(),
        };
        for (path, bytes, identity, cube) in [
            ("/index.html", b"fixture index".as_slice(), 1, false),
            ("/extensions/SugarCubes/main.js", b"fixture cube".as_slice(), 2, true),
        ] {
            super::manifest::enroll(
                &mut manifest,
                FrontendAsset {
                    path: path.into(),
                    bytes: bytes.len() as u64,
                    hash: Sha256::digest(bytes).into(),
                    identity: [identity; 24],
                },
                cube,
            )?;
        }
        state.manifest = Some(Arc::new(manifest));
        Ok(())
    }
    fn close(&self, state: &mut Inspection, _: &JobContext) -> Result<(), FrontendSourceError> {
        state.manifest = None;
        Ok(())
    }
}
/// Construct the real inert source owner using a fixed, bounded file-boundary fake.
/// Consumers must submit its finite inspection and consume its actual completion.
/// This fixture performs no filesystem operations and cannot construct a lease or readiness proof.
/// # Errors
/// Returns the ordinary installation-input validation error.
#[doc(hidden)]
pub fn owner() -> Result<InstalledFrontendOwner, FrontendSourceError> {
    let input = FrontendInstallationInput::new(
        r"E:\fixture\package".into(),
        r"E:\fixture\METADATA".into(),
        r"E:\fixture\SugarCubes".into(),
        r"E:\fixture\main.py".into(),
    )?;
    let mut owner = InstalledFrontendOwner::new(input);
    owner.port = Arc::new(FixedSource);
    Ok(owner)
}
