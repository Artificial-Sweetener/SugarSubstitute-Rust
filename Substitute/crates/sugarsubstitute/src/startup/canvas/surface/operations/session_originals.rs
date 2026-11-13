// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit original refusal must stop the actual owner before its descriptor-key fallback.
use super::*;
use crate::startup::canvas::session_source::{OriginalSpec, SessionSource};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

/// This deterministic owner test admits metadata only; it performs no storage work.
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

#[test]
fn unavailable_original_has_no_backend_or_fixture_retention_key() -> Result<(), Box<dyn Error>> {
    let memory = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let mut surface = NativeSurface::fixture(PathBuf::from("unused-owner-fixture"));
    surface.session_source =
        Some(SessionSource::new(vec![OriginalSpec::Unavailable], memory.claim(4 * 1024 * 1024)?));
    let identity = MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let file =
        CanvasFileReference::decode(br#"{"filename":"wide.png","subfolder":"","type":"output"}"#)?;
    let target = ComfyViewTarget::new(&file)?;
    assert!(surface.retention_key(identity, &target).is_none());
    Ok(())
}
