//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove a thumbnail reveal reaching a still-preparing native candidate takes effect on commit. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Mount a staged native candidate over real view and selection owners. */
function fixture() {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 500,
    bottom: 400,
    width: 500,
    height: 400,
    toJSON: () => ({}),
  });
  // jsdom cannot measure visibility; keep the observed allocation drawable once committed.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-reveal'));
  const root = document.createElement('section');
  document.body.append(root);
  const host = nativePreviewHost(false);
  const errors: unknown[] = [];
  const surface = new NativeCanvasAttachment(root, {
    address,
    clock: new WorkflowTestClock(),
    views,
    selection,
    frames: new CanvasFrameFixture(),
    inspections: new CanvasInspectionStore(),
    catalog: () => catalog,
    accepts: () => views.accepts(address),
    acceptsPreparation: () => views.acceptsPreparation(address),
    nativeSurface: () => host.host,
    connected: () => true,
    report: (error) => errors.push(error),
  });
  retirements.push(async () => {
    const retired = surface.dispose();
    for (const [index, record] of host.pending.entries())
      if (!record.settled)
        host.respond(
          index,
          host.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await retired;
  });
  const selected = views.read(view).selection;
  const other = catalog.entries.find((entry) => entry.source.id !== selected?.sourceId)!;
  const third = catalog.entries.find(
    (entry) => entry.source.id !== selected?.sourceId && entry !== other,
  )!;
  return {
    ...host,
    surface,
    views,
    view,
    selection,
    catalog,
    errors,
    target: other.items[0]!,
    alternative: third.items[0]!,
    /** Commit the staged candidate as the primary surface does once it is ready. */
    commit: () => {
      views.commitAttachment(address);
      surface.refresh();
    },
    /** Acknowledge every outstanding native ticket as the real host does. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 16; guard++) {
        await settle();
        const index = host.pending.findIndex((record) => !record.settled);
        if (index < 0) return;
        const call = host.calls[index]!;
        host.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? finalViewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
  };
}

/** Track whether a reveal promise has settled without awaiting it. */
function observed(reveal: Promise<boolean>) {
  const state: { settled: boolean; applied: boolean | null } = { settled: false, applied: null };
  void reveal.then((applied) => {
    state.settled = true;
    state.applied = applied;
  });
  return state;
}

test('a reveal clicked right after preparation starts takes effect once the candidate commits', async () => {
  const f = fixture();
  await settle();
  expect(f.calls.map(({ type }) => type)).toEqual(['surface.prepare']);
  const reveal = observed(f.surface.reveal(canvasRevealTarget(f.target)));
  await f.drain();
  await f.surface.ready;
  expect(reveal.settled).toBe(false);
  expect(f.views.read(f.view).selection?.sourceId).not.toBe(f.target.provenance.sourceId);
  f.commit();
  await f.drain();
  expect(reveal).toEqual({ settled: true, applied: true });
  expect(f.views.read(f.view).selection).toMatchObject({
    sourceId: f.target.provenance.sourceId,
    batchIndex: f.target.batchIndex,
  });
  expect(f.calls.filter(({ type }) => type === 'surface.select').at(-1)).toMatchObject({
    itemId: f.target.id,
  });
  expect(f.errors).toEqual([]);
});

test('a newer reveal replaces the retained one', async () => {
  const f = fixture();
  const first = observed(f.surface.reveal(canvasRevealTarget(f.alternative)));
  const second = observed(f.surface.reveal(canvasRevealTarget(f.target)));
  await settle();
  expect(first).toEqual({ settled: true, applied: false });
  await f.drain();
  f.commit();
  await f.drain();
  expect(second).toEqual({ settled: true, applied: true });
  expect(f.views.read(f.view).selection?.sourceId).toBe(f.target.provenance.sourceId);
});

test('a newer selection during preparation drops the retained reveal', async () => {
  const f = fixture();
  const reveal = observed(f.surface.reveal(canvasRevealTarget(f.target)));
  f.selection.apply(f.view, f.catalog, {
    type: 'select',
    selection: { sourceId: f.alternative.provenance.sourceId, batchIndex: 0 },
  });
  await f.drain();
  f.commit();
  await f.drain();
  expect(reveal).toEqual({ settled: true, applied: false });
  expect(f.views.read(f.view).selection?.sourceId).toBe(f.alternative.provenance.sourceId);
});

test('detaching a preparing candidate drops its retained reveal', async () => {
  const f = fixture();
  const reveal = observed(f.surface.reveal(canvasRevealTarget(f.target)));
  const retired = f.surface.dispose();
  await settle();
  expect(reveal).toEqual({ settled: true, applied: false });
  for (const [index, record] of f.pending.entries())
    if (!record.settled) f.respond(index, index ? 'applied' : 'cancelled');
  await retired;
  expect(await f.surface.reveal(canvasRevealTarget(f.target))).toBe(false);
});
