// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify one-time legacy adoption through real archive codec, metadata and selected scope.
use super::*;
use crate::input_documents::{InputRestorePlan, legacy};
use sugarsubstitute_contracts::desktop_session::{decode_checkpoint, wire::SessionCheckpoint};
fn checkpoint(mask: &str) -> TestResult<SessionCheckpoint> {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../third_party/sugarcubes-session/session_vectors.json"
    ))?;
    let mut checkpoint =
        decode_checkpoint(&serde_json::to_vec(&vectors["checkpoints"][0]["value"])?)?;
    for entry in &mut checkpoint.entries {
        entry.input = None;
        entry.document.graph_json = serde_json::json!({"extra":{"sugarcubes_input_canvas":{"version":1,"id":"workflow-real","route":"input","surface":"input","mask":mask}}}).to_string();
    }
    Ok(checkpoint)
}
fn archive(fixture: &Fixture, document: u64) -> TestResult<InputDocumentArchive> {
    let metadata = serde_json::json!({"version":1,"next_identity":document,"active":null,"workflows":[{"workflow":"workflow-real","surfaces":[{"key":"input","label":"Input","document":document,"base":{"kind":"file","reference":"{\"filename\":\"base.png\",\"type\":\"input\"}"},"loaded_base":null,"next_layer":3,"masks":[{"key":"mask","label":"Mask","layer":2,"color":[255,0,0],"opacity":0.5,"channel":"red","desired_initial":null,"loaded_initial":null}]}]}]});
    fixture.archive_named("input-1", 14, &serde_json::to_vec(&metadata)?)
}
fn seed(fixture: &Fixture, archive: InputDocumentArchive) -> TestResult<PathBuf> {
    let passive = fixture.store.directory().join("input-canvas");
    let target = passive.clone();
    let job = fixture.client.submit_blocking(
        &fixture.scope,
        fixture.spec(InputDocumentStore::save_reservation(&archive))?,
        move |context| files::retain(&target, &archive, &context),
    )?;
    match fixture.outcome(job)? {
        JobOutcome::Success(_) => Ok(passive),
        JobOutcome::Failure(cause) => Err(cause.into()),
        outcome => Err(format!("Legacy seed stopped: {:?}", outcome.kind()).into()),
    }
}
fn adopt(
    fixture: &Fixture,
    checkpoint: SessionCheckpoint,
) -> TestResult<Result<InputRestorePlan, StoreError>> {
    let user = fixture.store.directory().to_owned();
    let selected = user.join("selected-input");
    let job = fixture.client.submit_blocking(
        &fixture.scope,
        fixture.spec(256 * 1024 * 1024)?,
        move |context| legacy::adopt(&user, &selected, &checkpoint, &context),
    )?;
    match fixture.outcome(job)? {
        JobOutcome::Success(plan) => Ok(Ok(plan)),
        JobOutcome::Failure(cause) => Ok(Err(cause)),
        outcome => Err(format!("Legacy adoption stopped: {:?}", outcome.kind()).into()),
    }
}
#[test]
fn legacy_adoption_is_restart_stable_and_refuses_another_profile_or_backend() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = archive(&fixture, 1)?;
    let fingerprint = archive.fingerprint();
    let passive = seed(&fixture, archive)?;
    let original = std::fs::read(passive.join(CURRENT_ARCHIVE))?;
    for _ in 0..2 {
        let InputRestorePlan::Verified(source) = adopt(&fixture, checkpoint("mask")?)?? else {
            return Err("Actual matching archive was not adopted".into());
        };
        let job = source.submit_load(
            &fixture.client,
            &fixture.scope,
            fixture.spec(InputDocumentStore::load_reservation())?,
            fixture.memory.clone(),
        )?;
        let JobOutcome::Success(restored) = fixture.outcome(job)? else {
            return Err("Adopted immutable archive did not load".into());
        };
        assert_eq!(
            restored.archive.as_ref().map(|(_, archive)| archive.fingerprint()),
            Some(fingerprint)
        );
    }
    let marker = std::fs::read(passive.join("workspace-association.json"))?;
    let mut foreign = checkpoint("mask")?;
    foreign.scope.profile = "other".into();
    assert!(matches!(adopt(&fixture, foreign)?, Err(StoreError::Unverified)));
    let mut foreign = checkpoint("mask")?;
    foreign.scope.backend = "00000000-0000-4000-8000-000000000099".into();
    assert!(matches!(adopt(&fixture, foreign)?, Err(StoreError::Unverified)));
    assert_eq!(std::fs::read(passive.join(CURRENT_ARCHIVE))?, original);
    assert_eq!(std::fs::read(passive.join("workspace-association.json"))?, marker);
    Ok(())
}
#[test]
fn legacy_unmatched_mask_or_missing_real_document_preserves_unassociated_originals() -> TestResult {
    for document in [1, 2] {
        let fixture = Fixture::new()?;
        let passive = seed(&fixture, archive(&fixture, document)?)?;
        let original = std::fs::read(passive.join(CURRENT_ARCHIVE))?;
        let mask = if document == 1 { "foreign-mask" } else { "mask" };
        assert!(matches!(adopt(&fixture, checkpoint(mask)?)??, InputRestorePlan::Current));
        assert!(!passive.join("workspace-association.json").exists());
        assert_eq!(std::fs::read(passive.join(CURRENT_ARCHIVE))?, original);
    }
    Ok(())
}
#[test]
fn legacy_unknown_association_record_is_never_replaced_or_used_as_authority() -> TestResult {
    let fixture = Fixture::new()?;
    let passive = seed(&fixture, archive(&fixture, 1)?)?;
    let original = std::fs::read(passive.join(CURRENT_ARCHIVE))?;
    let unknown = b"{\"version\":2,\"future\":\"preserve\"}";
    std::fs::write(passive.join("workspace-association.json"), unknown)?;
    assert!(adopt(&fixture, checkpoint("mask")?)?.is_err());
    assert_eq!(std::fs::read(passive.join("workspace-association.json"))?, unknown);
    assert_eq!(std::fs::read(passive.join(CURRENT_ARCHIVE))?, original);
    Ok(())
}
#[test]
fn restored_native_reference_must_match_actual_workflow_surface_and_mask() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = archive(&fixture, 1)?;
    let fingerprint = archive.fingerprint();
    let passive = seed(&fixture, archive)?;
    let original = std::fs::read(passive.join(CURRENT_ARCHIVE))?;
    assert!(matches!(adopt(&fixture, checkpoint("mask")?)??, InputRestorePlan::Verified(_)));
    let selected = fixture.store.directory().join("selected-input");
    let signature: String = fingerprint.iter().map(|byte| format!("{byte:02x}")).collect();
    for changed in ["surface", "mask"] {
        let mut checkpoint = checkpoint("mask")?;
        for entry in &mut checkpoint.entries {
            let mut graph: serde_json::Value = serde_json::from_str(&entry.document.graph_json)?;
            graph["extra"]["sugarcubes_input_canvas"][changed] = "foreign-selection".into();
            entry.document.graph_json = graph.to_string();
            entry.input = Some(sugarsubstitute_contracts::desktop_session::wire::SessionInput {
                document_id: "input-1".into(),
                generation: 0,
                signature: signature.clone(),
                surface: Some("input".into()),
                mask: Some("mask".into()),
            });
        }
        let selected = selected.clone();
        let job = fixture.client.submit_blocking(
            &fixture.scope,
            fixture.spec(256 * 1024 * 1024)?,
            move |context| {
                crate::input_documents::workspace::restore(&selected, &checkpoint, &context)
            },
        )?;
        assert!(matches!(fixture.outcome(job)?, JobOutcome::Failure(StoreError::Unverified)));
    }
    assert_eq!(std::fs::read(passive.join(CURRENT_ARCHIVE))?, original);
    Ok(())
}
#[test]
fn new_backend_does_not_wait_for_or_adopt_another_namespaces_legacy_archive() -> TestResult {
    let fixture = Fixture::new()?;
    let passive = seed(&fixture, archive(&fixture, 1)?)?;
    assert!(matches!(adopt(&fixture, checkpoint("mask")?)??, InputRestorePlan::Verified(_)));
    let marker = std::fs::read(passive.join("workspace-association.json"))?;
    let original = std::fs::read(passive.join(CURRENT_ARCHIVE))?;
    let mut scope = checkpoint("mask")?.scope;
    scope.backend = "00000000-0000-4000-8000-000000000099".into();
    let user = fixture.store.directory().to_owned();
    let selected = user.join("new-backend-input");
    let job = fixture.client.submit_blocking(
        &fixture.scope,
        fixture.spec(128 * 1024)?,
        move |context| legacy::plan(&user, &selected, &scope, &context),
    )?;
    assert!(matches!(fixture.outcome(job)?, JobOutcome::Success(InputRestorePlan::Current)));
    assert_eq!(std::fs::read(passive.join("workspace-association.json"))?, marker);
    assert_eq!(std::fs::read(passive.join(CURRENT_ARCHIVE))?, original);
    Ok(())
}
