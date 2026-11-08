// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check immutable output receipts on actual admitted publication workers.
use super::*;
fn original() -> TestResult<SessionCheckpoint> {
    let vectors: serde_json::Value = serde_json::from_str(VECTORS)?;
    Ok(decode_checkpoint(&serde_json::to_vec(&vectors["checkpoints"][0]["value"])?)?)
}
fn verify(fixture: &Fixture, checkpoint: SessionCheckpoint) -> TestResult<Result<(), StoreError>> {
    let baseline = CheckpointBaseline {
        restored: Some(Arc::new(RestoredWorkspace {
            checkpoint: Some((CheckpointFile::Current, original()?)),
            rejected: vec![],
            protected: false,
            _memory: None,
        })),
        published: None,
    };
    let job = fixture.client.submit_blocking(&fixture.scope, fixture.spec()?, move |context| {
        baseline.verify(&checkpoint, &context)
    })?;
    match fixture.terminal(job)? {
        JobOutcome::Success(()) => Ok(Ok(())),
        JobOutcome::Failure(cause) => Ok(Err(cause)),
        outcome => Err(format!("Baseline verification stopped: {:?}", outcome.kind()).into()),
    }
}
#[test]
fn known_output_uuid_cannot_change_its_original_bytes_or_file_descriptor() -> TestResult {
    let fixture = Fixture::new()?;
    for field in ["digest", "file", "metadata"] {
        let mut checkpoint = original()?;
        let output = checkpoint.entries[0].outputs.first_mut().ok_or("Missing fixture output")?;
        match field {
            "digest" => output.digest = "f".repeat(64),
            "file" => output.file.filename = "replacement.png".into(),
            _ => output.metadata_embedded = !output.metadata_embedded,
        }
        assert!(matches!(verify(&fixture, checkpoint)?, Err(StoreError::Changed)));
    }
    Ok(())
}
#[test]
fn workspace_presentation_changes_do_not_rewrite_immutable_output_receipts() -> TestResult {
    let fixture = Fixture::new()?;
    let mut checkpoint = original()?;
    checkpoint.entries[0].document.name = "Renamed by user".into();
    checkpoint.entries[0].view.editor_width += 1;
    checkpoint.entries[0].outputs.reverse();
    verify(&fixture, checkpoint)??;
    Ok(())
}
#[test]
fn preceding_workspace_cannot_adopt_a_different_backend_namespace() -> TestResult {
    let fixture = Fixture::new()?;
    let mut checkpoint = original()?;
    checkpoint.scope.backend = "00000000-0000-4000-8000-000000000099".into();
    assert!(matches!(verify(&fixture, checkpoint)?, Err(StoreError::Protected)));
    Ok(())
}
