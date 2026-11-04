// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The canvas memory budget charges input documents for exactly as long as they live, and
//! work it cannot hold is refused while the canvas keeps running.

use super::*;
use sugarsubstitute_contracts::execution::MemoryClaim;

/// A second workflow, switched to and from like a person changing workflow tabs.
const OTHER_WORKFLOW: &str = "workflow-9a4e2f61-3b7d-4c58-8e19-5d6a7b8c9d0e";

impl Fixture {
    /// The canvas's resident lease, which input documents are charged to.
    fn resident(&self) -> Result<MemoryLease, Box<dyn Error>> {
        Ok(self.session.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone())
    }

    /// Claim everything the resident lease has left but `spare` bytes, as a full budget would.
    fn exhaust(&self, spare: u64) -> Result<MemoryClaim, Box<dyn Error>> {
        let memory = self.resident()?;
        Ok(memory.claim(memory.bytes() - memory.claimed_bytes() - spare)?)
    }

    /// Sync `surfaces` into `workflow` and settle.
    fn sync_workflow(
        &mut self,
        workflow: &str,
        surfaces: Vec<SurfaceSpec>,
    ) -> Result<InputReply, Box<dyn Error>> {
        let ticket = self.session.sync_inputs(workflow, surfaces)?;
        self.settle_inputs()?;
        let replies = self.session.take_input_replies();
        replies
            .into_iter()
            .find(|reply| reply.ticket == ticket)
            .ok_or_else(|| "The sync did not reply".into())
    }
}

/// An authored surface whose masks all load `pattern`, so every mask holds coverage tiles.
fn painted_surface(key: &str, base: &Path, pattern: &Path) -> Result<SurfaceSpec, Box<dyn Error>> {
    let masks = ["one", "two", "three"]
        .into_iter()
        .map(|name| super::mask(name, Some(file("input/pattern.png", pattern).0)))
        .collect::<Result<_, _>>()?;
    Ok(surface(key, SurfaceBase::File(file(&format!("input/{key}.png"), base).0), masks))
}

#[test]
fn switching_workflows_and_retiring_their_documents_returns_the_charge_to_its_baseline()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let directory = fixture._directory.path().to_path_buf();
        let extent = Extent::new(300, 200)?;
        let base = image(&directory, "base.png", extent, |x, y| [x as u8, y as u8, 7, 255])?;
        let pattern = image(&directory, "pattern.png", extent, |x, y| {
            [((x * 7 + y * 3) % 251) as u8, 0, 0, 255]
        })?;
        let memory = fixture.resident()?;
        fixture.settle_inputs()?;
        let baseline = memory.claimed_bytes();
        let mut charged = Vec::new();
        for round in 0..3 {
            for workflow in [WORKFLOW, OTHER_WORKFLOW] {
                let surfaces = (0..2)
                    .map(|index| painted_surface(&format!("image{index}"), &base, &pattern))
                    .collect::<Result<_, _>>()?;
                let reply = fixture.sync_workflow(workflow, surfaces)?;
                for index in 0..2 {
                    let synced = synced(&reply, &format!("image{index}"))?;
                    let snapshot = fixture.snapshot(synced.document)?;
                    for name in ["one", "two", "three"] {
                        let mask = coverage(&snapshot, layer(synced, name)?)?;
                        assert!(mask.charged_to(&memory), "round {round}: {name} is charged");
                    }
                }
            }
            // Both workflows' documents stay parked across the switch, and are charged.
            charged.push(memory.claimed_bytes());
            assert!(charged[round] > baseline, "round {round} charges its documents");
            if round > 0 {
                assert_eq!(charged[round], charged[0], "a repeated sync holds nothing more");
            }
        }
        for workflow in [WORKFLOW, OTHER_WORKFLOW] {
            fixture.sync_workflow(workflow, Vec::new())?;
        }
        assert_eq!(memory.claimed_bytes(), baseline, "retired documents release every charge");
    }
    fixture.finish().map(drop)
}

#[test]
fn a_stroke_the_budget_cannot_hold_is_cancelled_and_painting_resumes_once_memory_returns()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))?;
        let before = fixture.snapshot(image.document)?;
        // Less than one painted tile remains.
        let full = fixture.exhaust(16 * 1024)?;
        fixture.drag(&[[10.0, 10.0], [30.0, 20.0]])?;
        fixture.settle_inputs()?;
        assert!(fixture.session.failed.is_none(), "the canvas keeps running");
        let refused = fixture.snapshot(image.document)?;
        assert_eq!(coverage(&refused, first)?, coverage(&before, first)?, "nothing was painted");
        assert_eq!(fixture.session.edit_state()?.history, [false, false]);
        drop(full);
        fixture.drag(&[[10.0, 10.0], [30.0, 20.0]])?;
        fixture.settle_inputs()?;
        let painted = coverage(&fixture.snapshot(image.document)?, first)?;
        assert!(painted.value(20, 15) > 0, "the next stroke paints");
        assert!(painted.charged_to(&fixture.resident()?));
    }
    fixture.finish().map(drop)
}

#[test]
fn shape_results_are_charged_to_the_document_and_refused_when_it_cannot_hold_them()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        let memory = fixture.resident()?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::MaskRectangle))?;
        fixture.drag(&[[2.0, 2.0], [12.0, 12.0]])?;
        fixture.settle_inputs()?;
        let landed = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!(landed.value(7, 7), FULL_COVERAGE);
        // History keeps the job's result under the document's own lease.
        assert!(landed.charged_to(&memory));
        let revision = fixture.snapshot(image.document)?.revision();
        let full = fixture.exhaust(0)?;
        fixture.drag(&[[20.0, 2.0], [30.0, 12.0]])?;
        fixture.settle_inputs()?;
        assert!(fixture.session.failed.is_none(), "the canvas keeps running");
        let refused = fixture.snapshot(image.document)?;
        assert_eq!(refused.revision(), revision, "the refused shape changed nothing");
        drop(full);
        fixture.drag(&[[20.0, 2.0], [30.0, 12.0]])?;
        fixture.settle_inputs()?;
        assert_eq!(
            coverage(&fixture.snapshot(image.document)?, first)?.value(25, 7),
            FULL_COVERAGE
        );
    }
    fixture.finish().map(drop)
}

#[test]
fn a_base_change_the_budget_cannot_hold_is_answered_and_the_document_keeps_its_base()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let directory = fixture._directory.path().to_path_buf();
        let extent = Extent::new(40, 30)?;
        let red = image(&directory, "red.png", extent, |_, _| [255, 0, 0, 255])?;
        let authored = |fixture_file: &Path| -> Result<SurfaceSpec, Box<dyn Error>> {
            let base = SurfaceBase::File(file("input/red.png", fixture_file).0);
            Ok(surface("image", base, vec![super::mask("first", None)?]))
        };
        let created = fixture.sync(vec![authored(&red)?])?;
        let document = synced(&created, "image")?.document;
        let blank = || -> Result<SurfaceSpec, Box<dyn Error>> {
            let base = SurfaceBase::Blank(Extent::new(64, 48)?);
            Ok(surface("image", base, vec![super::mask("first", None)?]))
        };
        // The sync itself is accepted; the replacement backing is refused when it is built.
        let ticket = fixture.session.sync_inputs(WORKFLOW, vec![blank()?])?;
        let full = fixture.exhaust(0)?;
        fixture.settle_inputs()?;
        assert!(fixture.session.failed.is_none(), "the canvas keeps running");
        let reply = fixture
            .session
            .take_input_replies()
            .into_iter()
            .find(|reply| reply.ticket == ticket)
            .ok_or("The refused sync did not reply")?;
        assert!(
            matches!(
                reply.surfaces.as_slice(),
                [SurfaceOutcome::Failed { failure, .. }]
                    if failure.to_string().contains("memory budget")
            ),
            "{:?}",
            reply.surfaces
        );
        assert_eq!(fixture.snapshot(document)?.extent(), extent, "the old base stays");
        drop(full);
        let resynced = fixture.sync(vec![blank()?])?;
        assert_eq!(synced(&resynced, "image")?.extent, Extent::new(64, 48)?);
    }
    fixture.finish().map(drop)
}
