// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect observable checkpoint ordering with deterministic clocks and boundary values.
use super::*;
use sugarsubstitute_contracts::desktop_session::{decode_checkpoint, wire::SessionCheckpoint};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const FIXTURE: &str =
    include_str!("../../../../../Sugar/integrations/contracts/session_vectors.json");
fn at(milliseconds: u64) -> Moment {
    Moment::from_nanos(milliseconds * 1_000_000)
}
fn id(value: u16) -> Result<CheckpointId> {
    Ok(CheckpointId::new(format!("00000000-0000-4000-8000-{value:012x}"))?)
}
struct Fixture {
    owner: WorkspaceSession,
    ready: SessionMessage,
    checkpoint: SessionCheckpoint,
}
impl Fixture {
    fn new(restored: bool, protected: bool) -> Result<Self> {
        let vectors: serde_json::Value = serde_json::from_str(FIXTURE)?;
        let checkpoint =
            decode_checkpoint(&serde_json::to_vec(&vectors["checkpoints"][0]["value"])?)?;
        let fence: SessionFence = serde_json::from_value(vectors["currentFence"].clone())?;
        let mut owner = WorkspaceSession::new(
            checkpoint.scope.clone(),
            "http://127.0.0.1:8188".into(),
            fence.webview.clone(),
        )?;
        owner.loaded(
            restored.then(|| DurableCheckpoint {
                id: CheckpointId::new(checkpoint.id.clone()).unwrap_or_else(|_| unreachable!()),
                revision: checkpoint.revision,
            }),
            protected,
        )?;
        let ready = SessionMessage {
            protocol: "sugarcubes.desktop-session".into(),
            version: 1,
            request_id: id(90)?.as_str().into(),
            scope: checkpoint.scope.clone(),
            origin: "http://127.0.0.1:8188".into(),
            operation: SessionOperation::Ready {
                fence,
                capabilities: SessionCapabilities {
                    capture: true,
                    materialize: true,
                    chunk_transfer: true,
                    artifact_resolution: true,
                },
                committed: true,
                workflow_persistence: true,
            },
        };
        owner.observe(&ready, at(0))?;
        Ok(Self { owner, ready, checkpoint })
    }
    fn fence(&self) -> &SessionFence {
        let SessionOperation::Ready { fence, .. } = &self.ready.operation else { unreachable!() };
        fence
    }
    fn capture(&mut self, now: u64, number: u16) -> Result<CaptureTicket> {
        self.owner.capture(at(now), id(number)?)?.ok_or_else(|| "Expected capture".into())
    }
    fn content(&self, ticket: &CaptureTicket) -> SessionCheckpoint {
        let mut content = self.checkpoint.clone();
        content.id = ticket.checkpoint.as_str().into();
        content.revision = ticket.revision;
        content
    }
    fn advance(&mut self, now: u64) -> Result {
        let SessionOperation::Ready { fence, .. } = &mut self.ready.operation else {
            unreachable!()
        };
        fence.revision += 1;
        self.owner.observe(&self.ready, at(now))?;
        Ok(())
    }
}
#[test]
fn restored_workspace_suppresses_capture_until_complete_materialization() -> Result {
    let mut fixture = Fixture::new(true, false)?;
    assert!(fixture.owner.capture(at(250), id(100)?)?.is_none());
    fixture.owner.reconciled(&fixture.fence().clone())?;
    assert_eq!(fixture.capture(250, 100)?.revision, 10);
    Ok(())
}
#[test]
fn quiet_interval_and_continuous_dirty_limit_use_observed_clock() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    assert!(fixture.owner.capture(at(249), id(100)?)?.is_none());
    for now in (200..=1800).step_by(200) {
        fixture.advance(now)?;
    }
    assert_eq!(fixture.owner.deadline(), Some(at(2000)));
    assert!(fixture.owner.capture(at(1999), id(100)?)?.is_none());
    assert_eq!(fixture.capture(2000, 100)?.revision, 1);
    Ok(())
}
#[test]
fn capture_is_not_saved_until_matching_publication_terminal() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    let ticket = fixture.capture(250, 100)?;
    let content = fixture.content(&ticket);
    fixture.owner.admit_capture(&ticket, &content)?;
    assert!(fixture.owner.durable().is_none());
    assert!(fixture.owner.finished(&ticket, true, at(260)).is_err());
    fixture.owner.publish(&ticket, &content)?;
    let receipt =
        fixture.owner.finished(&ticket, true, at(260))?.ok_or("Missing durable receipt")?;
    assert_eq!(fixture.owner.durable(), Some(&receipt));
    assert!(fixture.owner.finished(&ticket, true, at(270)).is_err());
    Ok(())
}
#[test]
fn superseded_capture_cannot_publish_and_newest_observation_coalesces() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    let first = fixture.capture(250, 100)?;
    fixture.advance(260)?;
    fixture.advance(270)?;
    assert!(fixture.owner.capture(at(520), id(101)?)?.is_none());
    assert!(fixture.owner.publish(&first, &fixture.content(&first)).is_err());
    fixture.owner.finished(&first, false, at(520))?;
    let newest = fixture.capture(770, 101)?;
    assert_eq!(newest.fence.revision, 11);
    assert_eq!(newest.revision, 2);
    Ok(())
}
#[test]
fn every_binding_revision_and_runtime_generation_fences_publication() -> Result {
    for field in 0..6 {
        let mut fixture = Fixture::new(false, false)?;
        let ticket = fixture.capture(250, 100)?;
        let SessionOperation::Ready { fence, .. } = &mut fixture.ready.operation else {
            unreachable!()
        };
        fence.revision += 1;
        let binding = &mut fence.bindings[0];
        match field {
            0 => binding.generation += 1,
            1 => binding.graph_revision += 1,
            2 => binding.source_revision += 1,
            3 => binding.navigation_revision += 1,
            4 => binding.runtime_session.push('x'),
            _ => binding.entry_id = id(200)?.as_str().into(),
        }
        let observed = fixture.owner.observe(&fixture.ready, at(260));
        if field >= 4 {
            assert!(observed.is_err(), "A live runtime/occurrence cannot be reassigned");
            fixture.owner.publish(&ticket, &fixture.content(&ticket))?;
            continue;
        }
        observed?;
        assert!(fixture.owner.publish(&ticket, &fixture.content(&ticket)).is_err());
    }
    Ok(())
}
#[test]
fn same_observation_revision_cannot_change_binding_or_persistence() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    let original = fixture.ready.clone();
    let SessionOperation::Ready { workflow_persistence, .. } = &mut fixture.ready.operation else {
        unreachable!()
    };
    *workflow_persistence = false;
    assert!(fixture.owner.observe(&fixture.ready, at(10)).is_err());
    fixture.owner.observe(&original, at(10))?;
    assert!(fixture.owner.capture(at(250), id(100)?)?.is_some());
    Ok(())
}
#[test]
fn unsupported_original_is_protected_even_with_verified_previous() -> Result {
    let mut fixture = Fixture::new(true, true)?;
    fixture.owner.reconciled(&fixture.fence().clone())?;
    assert_eq!(fixture.owner.capture(at(250), id(100)?), Err(WorkspaceError::Protected));
    assert_eq!(fixture.owner.durable().map(|value| value.revision), Some(9));
    Ok(())
}
#[test]
fn final_capture_precedes_close_and_keeps_original_cutoff() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    fixture.owner.begin_close(at(10), at(1000))?;
    fixture.owner.begin_close(at(11), at(2000))?;
    assert!(!fixture.owner.close_ready(at(11)));
    assert!(fixture.owner.changed(at(12)).is_err());
    let ticket = fixture.capture(12, 100)?;
    assert!(ticket.closing);
    fixture.owner.publish(&ticket, &fixture.content(&ticket))?;
    assert!(!fixture.owner.close_ready(at(13)));
    fixture.owner.finished(&ticket, true, at(14))?;
    assert!(fixture.owner.close_ready(at(14)));
    let mut timed = Fixture::new(false, false)?;
    timed.owner.begin_close(at(10), at(1000))?;
    timed.owner.begin_close(at(11), at(2000))?;
    assert!(timed.owner.close_ready(at(1000)));
    Ok(())
}
#[test]
fn close_during_old_save_captures_final_once_after_terminal_ack() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    let first = fixture.capture(250, 100)?;
    fixture.owner.publish(&first, &fixture.content(&first))?;
    fixture.advance(260)?;
    fixture.owner.begin_close(at(270), at(1000))?;
    assert!(fixture.owner.capture(at(270), id(101)?)?.is_none());
    fixture.owner.finished(&first, true, at(280))?;
    assert!(!fixture.owner.close_ready(at(280)));
    let final_ticket = fixture.capture(280, 101)?;
    assert_eq!(final_ticket.fence.revision, 10);
    assert!(final_ticket.closing);
    Ok(())
}

#[test]
fn rename_preserves_occurrence_and_duplicate_cannot_steal_its_runtime() -> Result {
    let mut fixture = Fixture::new(false, false)?;
    fixture.advance(10)?;
    let mut duplicated = fixture.ready.clone();
    let SessionOperation::Ready { fence, .. } = &mut duplicated.operation else {
        return Err("Missing Ready".into());
    };
    fence.revision += 1;
    let mut binding = fence.bindings[0].clone();
    binding.entry_id = id(200)?.as_str().into();
    fence.bindings.push(binding.clone());
    assert!(fixture.owner.observe(&duplicated, at(20)).is_err());
    binding.runtime_session = "fresh-duplicate-runtime".into();
    let SessionOperation::Ready { fence, .. } = &mut duplicated.operation else {
        return Err("Missing Ready".into());
    };
    *fence.bindings.last_mut().ok_or("Missing duplicate")? = binding;
    fixture.owner.observe(&duplicated, at(20))?;
    Ok(())
}
