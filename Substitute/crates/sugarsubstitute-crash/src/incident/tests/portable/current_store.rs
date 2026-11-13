// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Passive old-namespace history remains independent of a genuine fresh current-store operation.
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticStoreOpening, fixture::DiagnosticStorageFixture,
};

#[test]
fn next_current_capture_uses_its_own_genesis_while_old_history_still_exports() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let expected = b"original old-namespace attachment";
        let old_key = source.retained(expected)?;
        let original = source.original_bytes();
        let mut source = source.copied();
        let mut history =
            project_portable_committed(&mut source, NamespaceId::new([1; 32]), now()?, &context)?;
        let fixture = DiagnosticStorageFixture::new()?;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([9; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        };
        let operation_key = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?);
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(operation_key, &context)?;
        let DiagnosticStoreOpening::Created(mut genesis) =
            fixture.resolve_opening(&mut storage, &operation, &context)?
        else {
            return Err("New current namespace did not produce genuine fixture genesis".into());
        };
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut repository = IncidentRepository::new(storage, operation, &context)?;
        repository.initialize(&mut genesis, &context)?;
        let seed = IncidentSeed::new(
            IncidentKey::new(IncidentSubject::Desktop(RunKey {
                owner,
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            })),
            IncidentRevision::new(1)?,
            BuildIdentity::new("current_build")?,
            IncidentUtc::from_unix_seconds(100)?,
            IncidentStage::Startup,
        )?;
        let summary = IncidentSummary::new(
            seed.clone(),
            IncidentRevision::new(2)?,
            base_summary()?.summary().facts().clone(),
        )?;
        let preflight = repository.preflight_publication(
            seed,
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(262_144, 1, 1, 131_072)?,
            now()?,
            &context,
        )?;
        let mut ticket =
            repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)?;
        let committed = repository.commit(&mut ticket, summary.clone(), &context)?;
        assert_eq!(committed.key().subject().coordinator().namespace, owner.namespace);
        assert_ne!(
            committed.key().subject().coordinator().namespace,
            old_key.subject().coordinator().namespace
        );
        let loaded = repository.load(committed.key(), &context)?;
        assert_eq!(loaded.summary(), &summary);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        let mut exported = Vec::new();
        history.export_attachment(old_key, AttachmentKind::Fault, &mut exported, &context)?;
        assert_eq!(exported, expected);
        drop(history);
        assert!(source.unchanged(&original));
        Ok(())
    })
}
