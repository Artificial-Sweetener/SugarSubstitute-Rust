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
/** Prove a final the host cannot decode shows the load-error state instead of a released surface. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
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

/** Mount a staged or committed native attachment whose host cannot decode one result. */
function fixture(staged: boolean) {
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
  // jsdom cannot measure visibility; keep the observed allocation drawable.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-select-error'));
  if (!staged) views.commitAttachment(address);
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
    acceptsPreparation: () => staged && views.acceptsPreparation(address),
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
  let undecodable: string | null = null;
  return {
    ...host,
    surface,
    errors,
    selected: () => {
      const selected = views.read(view).selection;
      return catalog.entries.find((entry) => entry.source.id === selected?.sourceId)!.items[0]!;
    },
    other: (item: WorkflowOutputItem) =>
      catalog.entries.find((entry) => entry.items[0]!.id !== item.id)!.items[0]!,
    undecodable: (item: WorkflowOutputItem) => {
      undecodable = item.id;
    },
    select: (item: WorkflowOutputItem) =>
      selection.apply(view, catalog, {
        type: 'select-source',
        selection: { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex },
      }),
    commit: () => {
      views.commitAttachment(address);
      surface.refresh();
    },
    /** Acknowledge every ticket, refusing selection of the result the host cannot decode. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 24; guard++) {
        await settle();
        const index = host.pending.findIndex((record) => !record.settled);
        if (index < 0) return;
        const call = host.calls[index]!;
        if (call.type === 'surface.select' && call.itemId === undecodable)
          host.pending[index]!.resolve({ status: 'error', message: 'Image decode failed.' });
        else
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

test.each([true, false])(
  'an undecodable final shows the load error and a later selection recovers (staged: %s)',
  async (staged) => {
    const f = fixture(staged);
    const failed = f.selected();
    f.undecodable(failed);
    const ready = f.surface.ready.then(
      () => 'ready',
      () => 'rejected',
    );
    await f.drain();
    expect(await ready).toBe('ready');
    if (staged) {
      f.commit();
      await f.drain();
    }
    const snapshot = f.surface.port.read();
    expect(snapshot.resource).toMatchObject({
      state: 'error',
      message: 'The image could not be loaded.',
    });
    // The shared overlay shows the load error rather than leaving an empty canvas.
    expect(document.querySelector('.sc-output-status')?.textContent).toContain(
      'The image could not be loaded.',
    );
    expect(f.calls.some(({ type }) => type === 'surface.release')).toBe(false);
    expect(f.surface.viewport.classList.contains('sc-output-native')).toBe(false);
    const next = f.other(failed);
    f.select(next);
    await f.drain();
    expect(f.calls.filter(({ type }) => type === 'surface.select').at(-1)).toMatchObject({
      itemId: next.id,
    });
    expect(f.calls.some(({ type }) => type === 'surface.commit')).toBe(true);
    expect(f.surface.port.read().resource.state).toBe('ready');
    expect(f.surface.viewport.classList.contains('sc-output-native')).toBe(true);
    expect(f.calls.some(({ type }) => type === 'surface.release')).toBe(false);
  },
);
