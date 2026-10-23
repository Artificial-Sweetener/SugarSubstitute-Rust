// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run independent snapshot generations through the actual inspector and execution owner.
use super::*;
struct EditedSource(AtomicU64);
impl SourcePort for EditedSource {
    fn inspect(
        &self,
        _: &FrontendInstallationInput,
        retained: &mut Inspection,
        _: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        let revision = self.0.load(Ordering::Acquire) as u8;
        let mut current = inventory();
        let mut source = asset("/extensions/SugarCubes/edited.js", revision, u64::from(revision));
        source.hash = [revision; 32];
        manifest::enroll(&mut current, source, true)?;
        retained.manifest = Some(Arc::new(current));
        Ok(())
    }
    fn close(&self, _: &mut Inspection, _: &JobContext) -> Result<(), FrontendSourceError> {
        Ok(())
    }
}
#[test]
fn fresh_snapshot_observes_edits_without_retiring_original_client_or_copying_old_proof()
-> TestResult {
    let mut rig = Harness::new(false, false)?;
    let boundary = Arc::new(EditedSource(AtomicU64::new(1)));
    rig.owner.port = boundary.clone();
    rig.inspect()?;
    let original = rig.owner.take_inspection()?.ok_or("Original snapshot absent")?;
    boundary.0.store(2, Ordering::Release);
    let mut fresh = rig.owner.fresh_snapshot();
    assert!(fresh.take_inspection()?.is_none());
    fresh.submit_inspection(&rig.client, &rig.scope, Harness::spec(2, 1000)?)?;
    rig.wake.until(&rig.client, || {
        fresh
            .pending
            .as_ref()
            .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_)))
    })?;
    let edited = fresh.take_inspection()?.ok_or("Fresh snapshot absent")?;
    assert_eq!(original.manifest().assets()[0].bytes(), 1);
    assert_eq!(edited.manifest().assets()[0].bytes(), 2);
    assert_ne!(original.manifest().assets()[0].sha256(), edited.manifest().assets()[0].sha256());
    original.validate_live()?;
    edited.validate_live()?;
    let mut close =
        rig.owner.submit_close(&rig.client, &rig.scope, Harness::close_spec(3, 2000)?)?;
    rig.wake.until(&rig.client, || matches!(close.status().phase, JobPhase::Terminal(_)))?;
    assert!(matches!(
        close.try_take_completion().ok_or("Original close absent")?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(original.validate_live().is_err());
    edited.validate_live()?;
    let mut close = fresh.submit_close(&rig.client, &rig.scope, Harness::close_spec(4, 2000)?)?;
    rig.wake.until(&rig.client, || matches!(close.status().phase, JobPhase::Terminal(_)))?;
    assert!(matches!(
        close.try_take_completion().ok_or("Fresh close absent")?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(edited.validate_live().is_err());
    Ok(())
}
