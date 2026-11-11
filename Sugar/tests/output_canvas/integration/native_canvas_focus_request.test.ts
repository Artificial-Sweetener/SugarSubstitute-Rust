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
/** Prove a card opening Input gives the committed native canvas keyboard focus and its keys. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasRoutePort } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasRoute.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { clientRect } from '../../canvas_editing/support/CanvasEditOverlayFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';
import type { CanvasRouteState } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasRoute.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Mount a native attachment whose route raises focus requests on demand. */
function fixture(staged: boolean, options: { route?: CanvasRouteState; empty?: boolean } = {}) {
  // jsdom cannot measure visibility; keep the observed allocation drawable.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  const catalog = options.empty ? canvasCatalogFixture(1, 0, []) : canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-focus-request'));
  if (!staged) views.commitAttachment(address);
  const root = document.createElement('section');
  const card = document.createElement('button');
  document.body.append(card, root);
  const requests = new Set<() => void>();
  const route: CanvasRoutePort = {
    read: () => options.route ?? null,
    subscribe: () => () => {},
    choose: () => {},
    subscribeFocus: (listener) => {
      requests.add(listener);
      return () => requests.delete(listener);
    },
  };
  const host = nativePreviewHost(options.empty === true);
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
    route,
    report: () => {},
  });
  let retired = false;
  const retire = async (): Promise<void> => {
    if (retired) return;
    retired = true;
    const released = surface.dispose();
    for (const [index, record] of host.pending.entries())
      if (!record.settled)
        host.respond(
          index,
          host.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await released;
  };
  retirements.push(retire);
  return {
    host,
    surface,
    card,
    retire,
    requests,
    request: () => {
      for (const listener of requests) listener();
    },
  };
}

test('a focus request moves keyboard focus from the card to the canvas viewport', () => {
  const f = fixture(false);
  f.card.focus();
  f.request();
  expect(document.activeElement).toBe(f.surface.viewport);
});

test('a staged candidate leaves focus alone and a retired attachment stops listening', async () => {
  const staged = fixture(true);
  staged.card.focus();
  staged.request();
  expect(document.activeElement).toBe(staged.card);
  await staged.retire();
  expect(staged.requests.size).toBe(0);
});

const inputRoute: CanvasRouteState = {
  route: 'input',
  inputAvailable: true,
  labels: { input: 'Input', output: 'Output', group: 'Canvas route' },
};

test('zoom and pan keys move the Input document camera while the Output route has no result', async () => {
  const f = fixture(false, { route: inputRoute, empty: true });
  f.host.ready('observing');
  await settle();
  for (let index = 0; index < 3; index++) {
    f.host.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
  expect(f.host.calls.map((call) => call.type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.commit',
  ]);
  f.request();
  const press = (key: string) =>
    f.surface.viewport.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  press('+');
  await settle();
  expect(f.host.calls[3]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'zoom', factor: 1.25 },
  });
  f.host.respond(3, 'applied', {
    mode: 'custom',
    physicalScale: 1.25,
    dimensions: { width: 512, height: 512 },
  });
  await settle();
  jest
    .spyOn(f.surface.viewport, 'getBoundingClientRect')
    .mockReturnValue(clientRect({ left: 0, top: 0, width: 500, height: 400 }));
  press('ArrowLeft');
  await settle();
  expect(f.host.calls[4]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'pan', units: 'viewport' },
  });
});
