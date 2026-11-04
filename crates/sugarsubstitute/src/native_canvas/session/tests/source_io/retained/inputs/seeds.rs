// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A region's seed file lands in its mask wherever presentation custody holds the document while
//! the file loads, including right after a restart restored the Input route, and a region that
//! was painted after its seed landed keeps its paint when the workflow syncs its files again.

use super::*;
use crate::input_documents::InputDocumentStore;
use crate::native_canvas::session::inputs::{InitialOutcome, InputTicket};
use std::sync::PoisonError;

/// The workflow a test opens after the first one, as opening a workflow file again does.
const OPENED: &str = "workflow-5d2c9a71-0e4b-4f8a-b3c6-7a1e2f9d8c40";

/// The size of the regional surface and of its seed files.
fn extent() -> Result<Extent, Box<dyn Error>> {
    Ok(Extent::new(4, 2)?)
}

/// Acquisition progress of a [`GatedFile`].
#[derive(Default)]
struct Gate {
    entered: bool,
    open: bool,
    cancelled: bool,
    settled: bool,
}

/// A ComfyUI input file whose acquisition waits until the test opens its gate, so the file is
/// still loading while the test moves its document.
struct GatedFile {
    path: PathBuf,
    state: Mutex<Gate>,
    changed: Condvar,
}

impl GatedFile {
    fn new(path: &Path) -> Arc<Self> {
        Arc::new(Self {
            path: path.to_path_buf(),
            state: Mutex::new(Gate::default()),
            changed: Condvar::new(),
        })
    }

    /// Wait until an import job is acquiring the file, bounding a stalled worker.
    fn wait_entered(&self) -> Result<(), Box<dyn Error>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.entered)
            .map_err(|cause| io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !state.entered {
            return Err("The seed file was never acquired".into());
        }
        Ok(())
    }

    /// Let the acquisition finish; opening an open gate again is harmless.
    fn open(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.open = true;
        self.changed.notify_all();
    }
}

impl SourceAcquirer for GatedFile {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("Gate state poisoned"))?;
        state.entered = true;
        self.changed.notify_all();
        while !state.open && !state.cancelled {
            state =
                self.changed.wait(state).map_err(|_| io::Error::other("Gate state poisoned"))?;
        }
        state.settled = true;
        if state.cancelled {
            return Err(io::ErrorKind::Interrupted.into());
        }
        Ok(SelectedOriginals::single(ImageSource::file(self.path.clone())))
    }

    fn cancel(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.cancelled = true;
        self.changed.notify_all();
    }

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("Gate state poisoned"))?;
        state.settled = true;
        Ok(())
    }

    fn settled(&self) -> bool {
        self.state.lock().map(|state| state.settled).unwrap_or(false)
    }
}

/// Open a gate however the test ends, so no worker outlives the fixture.
struct OpenOnDrop(Arc<GatedFile>);

impl Drop for OpenOnDrop {
    fn drop(&mut self) {
        self.0.open();
    }
}

/// Seed files covering the left (Sky) and the right (Tree) half of the region.
fn seed_files(directory: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let half = |left: bool| {
        move |x: u32, _: u32| if (x < 2) == left { [255, 255, 255, 255] } else { [0, 0, 0, 255] }
    };
    let sky = image(directory, "sky.png", extent()?, half(true))?;
    let tree = image(directory, "tree.png", extent()?, half(false))?;
    Ok((sky, tree))
}

/// A blank-based regional surface with one mask per region, seeded by the given files.
fn regions(sky: Option<ComfyFile>, tree: Option<ComfyFile>) -> Result<SurfaceSpec, Box<dyn Error>> {
    Ok(surface(
        "regions",
        SurfaceBase::Blank(extent()?),
        vec![mask("sky", sky)?, mask("tree", tree)?],
    ))
}

/// What a synced mask reports about its seed file.
fn initial<'a>(surface: &'a SyncedSurface, key: &str) -> Option<&'a InitialOutcome> {
    surface.masks.iter().find(|mask| &*mask.key == key).and_then(|mask| mask.initial.as_ref())
}

/// Whether a mask covers exactly the left or the right half of the region.
fn holds_half(
    snapshot: &DocumentSnapshot,
    layer: LayerId,
    left: bool,
) -> Result<bool, Box<dyn Error>> {
    let values = coverage(snapshot, layer)?;
    Ok((0..2)
        .all(|y| (0..4).all(|x| values.value(x, y) == if (x < 2) == left { u16::MAX } else { 0 })))
}

/// Require both regions of `surface` to hold their seed files.
fn assert_seeded(
    fixture: &Fixture,
    surface: &SyncedSurface,
    phase: &str,
) -> Result<(), Box<dyn Error>> {
    let snapshot = fixture.snapshot(surface.document)?;
    assert!(holds_half(&snapshot, layer(surface, "sky")?, true)?, "{phase}: Sky holds sky.png");
    assert!(holds_half(&snapshot, layer(surface, "tree")?, false)?, "{phase}: Tree holds tree.png");
    Ok(())
}

impl Fixture {
    /// Start a session whose input documents persist below `anchor`, or below the fixture's own
    /// directory, and observe its startup restore before any workflow syncs.
    fn persisted(anchor: Option<&Path>) -> Result<Self, Box<dyn Error>> {
        let (mut session, clock, directory) = session()?;
        let root = anchor.unwrap_or(directory.path()).join("input-canvas");
        session.persist_inputs(InputDocumentStore::new(root)?);
        let mut fixture = Self::launch(session, clock, directory, false, RETAINED_DOCUMENT_BYTES)?;
        fixture.poll_until("the restored input documents", false, Session::inputs_restored)?;
        // Autosave takes its baseline on the poll after the restore; later syncs are changes.
        fixture.poll_until("the autosave baseline", false, |_| true)?;
        Ok(fixture)
    }

    /// Poll until `reached` holds. Completions are awaited while canvas work is in flight, and
    /// for an input job only when `inputs` says so, since a held seed import never completes on
    /// its own.
    fn poll_until(
        &mut self,
        what: &str,
        inputs: bool,
        reached: impl Fn(&Session) -> bool,
    ) -> Result<(), Box<dyn Error>> {
        for _ in 0..500 {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if let Some(cause) = &self.session.failed {
                return Err(format!("Canvas failed before {what}: {cause}").into());
            }
            if reached(&self.session) {
                return Ok(());
            }
            let session = &self.session;
            if session.loading.is_some()
                || session.rendering.is_some()
                || session.displaying.is_some()
                || session.retirement.is_some()
                || session.acquisition_retirement.is_some()
                || !session.inputs_restored()
                || inputs && session.inputs.running()
            {
                self.wake.wait_after(observed).map_err(|cause| format!("{what}: {cause}"))?;
            }
        }
        Err(format!("The canvas never reached {what}").into())
    }

    /// Show `surface` of `workflow` while its first seed file is still being acquired.
    fn show_while_seeding(
        &mut self,
        workflow: &str,
        surface: &str,
        gate: &GatedFile,
    ) -> Result<InputDocumentId, Box<dyn Error>> {
        let earlier: Vec<_> =
            self.session.input_revisions().into_iter().map(|(document, _)| document).collect();
        let created = |session: &Session| {
            session.input_revisions().into_iter().find(|(document, _)| !earlier.contains(document))
        };
        self.session.activate_input(workflow, surface, None)?;
        // The document's creation completes; its first seed import starts on the same poll.
        self.poll_until("the seeding document created", true, |session| {
            created(session).is_some()
        })?;
        let (document, _) = created(&self.session).ok_or("The seeding document vanished")?;
        gate.wait_entered()?;
        self.poll_until("the seeding document shown", false, |session| {
            session.document.input == Some(document)
        })?;
        assert!(self.session.inputs.running(), "the first seed file is still loading");
        Ok(document)
    }

    /// Withdraw the shown document into a document preparation, as a newly attached canvas
    /// does when the page opens another workflow tab.
    fn prepare(&mut self) -> Result<DocumentToken, Box<dyn Error>> {
        for _ in 0..500 {
            if let Some(token) = self.session.prepare_document()? {
                return Ok(token);
            }
            self.poll_until("a settled document", false, |session| {
                session.rendering.is_none()
                    && session.displaying.is_none()
                    && session.loading.is_none()
                    && session.retirement.is_none()
                    && session.pending_retirement.is_none()
            })?;
        }
        Err("The shown document never settled for a preparation".into())
    }

    /// Roll a preparation back once its candidate's work has been observed.
    fn roll_back(&mut self, token: DocumentToken) -> Result<(), Box<dyn Error>> {
        self.session.restore_document(token)?;
        self.poll_until("the rolled-back document", false, |session| {
            !session.document_transaction_pending(token)
        })?;
        self.settle_inputs()
    }

    /// Withdraw the shown, still seeding document into a document preparation, let its seed
    /// files finish loading and sync the workflow again without files, as the page does next.
    /// Both syncs must report both seeds loaded; returns the preparation and the synced surface.
    fn seed_while_prepared(
        &mut self,
        workflow: &str,
        opened: InputTicket,
        gate: &GatedFile,
    ) -> Result<(DocumentToken, SyncedSurface), Box<dyn Error>> {
        let shown = self.session.document.input;
        let token = self.prepare()?;
        assert_eq!(self.session.document.input, None, "the preparation holds the document");
        gate.open();
        self.settle_inputs()?;
        let later = self.session.sync_inputs(workflow, vec![regions(None, None)?])?;
        self.settle_inputs()?;
        let replies = self.session.take_input_replies();
        let find = |ticket| replies.iter().find(|reply: &&InputReply| reply.ticket == ticket);
        let later = find(later).ok_or("The later sync did not reply")?;
        let seeded = synced(later, "regions")?.clone();
        assert_eq!(Some(seeded.document), shown);
        for key in ["sky", "tree"] {
            let outcome = initial(&seeded, key);
            assert!(matches!(outcome, Some(InitialOutcome::Loaded)), "{key}: {outcome:?}");
        }
        let opened = find(opened).ok_or("The opening sync did not reply")?;
        let first = synced(opened, "regions")?;
        assert_eq!(first.document, seeded.document);
        for key in ["sky", "tree"] {
            let outcome = initial(first, key);
            assert!(matches!(outcome, Some(InitialOutcome::Loaded)), "opening {key}: {outcome:?}");
        }
        assert_seeded(self, &seeded, "while prepared")?;
        Ok((token, seeded))
    }

    /// Paint every pixel of `layer` through the document's own edit history.
    fn paint_all(
        &mut self,
        document: InputDocumentId,
        layer: LayerId,
    ) -> Result<(), Box<dyn Error>> {
        let memory = self.session.resources.as_ref().ok_or("Canvas storage absent")?.memory.clone();
        let (state, _) = self.session.input_document_mut(document).ok_or("Document absent")?;
        let scene = state.scene.as_mut().ok_or("Scene absent")?;
        let snapshot = scene.document.snapshot();
        let current = snapshot
            .layers()
            .iter()
            .find(|candidate| candidate.id() == layer)
            .ok_or("Mask absent")?;
        let LayerContent::Coverage { coverage, appearance } = current.content() else {
            return Err("Mask is not coverage".into());
        };
        let mut painted = coverage.clone();
        painted.update(extent()?.region(), &memory, |_, _, _| u16::MAX)?;
        let content = LayerContent::Coverage { coverage: painted, appearance: *appearance };
        scene.document.apply(sugarsubstitute_canvas::DocumentEdit::Replace(
            current.clone().with_content(content),
        ))?;
        Ok(())
    }
}

/// Opening a workflow while the Input route shows: the opened surface is shown at once and a
/// newly attached canvas withdraws it while its seed files still load.
#[test]
fn seed_files_land_while_a_document_preparation_holds_the_shown_document()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let (sky_path, tree_path) = seed_files(&directory)?;
        let gate = GatedFile::new(&sky_path);
        let _open = OpenOnDrop(gate.clone());
        let sky = ComfyFile { reference: Arc::from("input/sky.png"), acquirer: gate.clone() };
        let (tree, _) = file("input/tree.png", &tree_path);
        let ticket =
            fixture.session.sync_inputs(WORKFLOW, vec![regions(Some(sky), Some(tree))?])?;
        let shown = fixture.show_while_seeding(WORKFLOW, "regions", &gate)?;
        let (token, seeded) = fixture.seed_while_prepared(WORKFLOW, ticket, &gate)?;
        // Rolling the preparation back shows the seeded document again.
        fixture.roll_back(token)?;
        assert_eq!(fixture.session.document.input, Some(shown));
        assert_seeded(&fixture, &seeded, "after the rollback")?;
        // A further sync naming no file changes nothing.
        let revision = fixture.snapshot(shown)?.revision();
        fixture.sync(vec![regions(None, None)?])?;
        assert_eq!(fixture.snapshot(shown)?.revision(), revision);
    }
    let _directory = fixture.finish()?;
    Ok(())
}

/// A restart restores the previous workflow's Input route; its tab syncs its files again
/// without replacing a painted region, and a workflow opened next seeds its regions although
/// the newly attached canvas withdraws the shown document while the files load.
#[test]
fn after_a_restart_an_opened_workflow_seeds_its_regions_and_painted_regions_keep_their_paint()
-> Result<(), Box<dyn Error>> {
    let mut first = Fixture::persisted(None)?;
    let (sky_path, tree_path) = seed_files(first._directory.path())?;
    let restored = {
        let reply = first.sync(vec![regions(
            Some(file("input/sky.png", &sky_path).0),
            Some(file("input/tree.png", &tree_path).0),
        )?])?;
        let restored = synced(&reply, "regions")?.clone();
        assert_seeded(&first, &restored, "first open")?;
        first.paint_all(restored.document, layer(&restored, "sky")?)?;
        first.session.activate_input(WORKFLOW, "regions", Some("sky"))?;
        first.settle_inputs()?;
        assert_eq!(first.session.document.input, Some(restored.document));
        restored
    };
    let anchor = first.finish()?;

    let mut second = Fixture::persisted(Some(anchor.path()))?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        second.poll_until("the restored Input route", false, |session| {
            session.document.input == Some(restored.document)
        })?;
        let (sky, sky_source) = file("input/sky.png", &sky_path);
        let (tree, tree_source) = file("input/tree.png", &tree_path);
        let resynced = second.sync(vec![regions(Some(sky), Some(tree))?])?;
        assert_eq!(synced(&resynced, "regions")?.document, restored.document);
        assert_eq!(sky_source.acquisitions.load(Ordering::SeqCst), 0, "a landed seed stays");
        assert_eq!(tree_source.acquisitions.load(Ordering::SeqCst), 0);
        let snapshot = second.snapshot(restored.document)?;
        let sky_layer = layer(&restored, "sky")?;
        assert_eq!(coverage(&snapshot, sky_layer)?.uniform(), Some(u16::MAX), "paint is kept");
        assert!(holds_half(&snapshot, layer(&restored, "tree")?, false)?);

        let gate = GatedFile::new(&sky_path);
        let _open = OpenOnDrop(gate.clone());
        let sky = ComfyFile { reference: Arc::from("input/sky.png"), acquirer: gate.clone() };
        let (tree, _) = file("input/tree.png", &tree_path);
        let ticket = second.session.sync_inputs(OPENED, vec![regions(Some(sky), Some(tree))?])?;
        let opened = second.show_while_seeding(OPENED, "regions", &gate)?;
        assert_ne!(opened, restored.document);
        let (token, seeded) = second.seed_while_prepared(OPENED, ticket, &gate)?;
        second.session.promote_empty_document(token)?;
        second.settle_inputs()?;
        assert_seeded(&second, &seeded, "after the attachment committed")?;
        let snapshot = second.snapshot(restored.document)?;
        assert_eq!(coverage(&snapshot, sky_layer)?.uniform(), Some(u16::MAX));
    }
    second.finish()?;
    drop(anchor);
    Ok(())
}

/// A seed file whose surface never loaded cannot land; the sync still replies, reporting the
/// surface's failure, instead of waiting for a later sync.
#[test]
fn a_seed_for_a_surface_that_failed_to_load_still_answers_its_sync() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let directory = fixture._directory.path().to_path_buf();
        let (sky_path, _) = seed_files(&directory)?;
        let (base, _) = file("input/missing.png", &directory.join("missing.png"));
        let (sky, _) = file("input/sky.png", &sky_path);
        let reply = fixture.sync(vec![surface(
            "regions",
            SurfaceBase::File(base),
            vec![mask("sky", Some(sky))?],
        )])?;
        assert!(
            matches!(&reply.surfaces[..], [SurfaceOutcome::Failed { key, .. }] if &**key == "regions")
        );
    }
    let _directory = fixture.finish()?;
    Ok(())
}
