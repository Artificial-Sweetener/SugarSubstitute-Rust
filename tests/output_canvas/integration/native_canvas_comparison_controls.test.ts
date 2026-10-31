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
/** Drive shared A/B controls through the real selection owner onto a fake native host. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
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

const comparing: Partial<CanvasSurfaceCapabilities> = {
  viewportCommands: ['fit', 'native', 'zoom', 'pan', 'wipe'],
  comparison: true,
};
const pairViewport: CanvasSurfaceViewportState = {
  ...finalViewport,
  secondaryDimensions: { width: 512, height: 384 },
};

/** Mount real view, selection, port and presentation owners over a held native host. */
function fixture(capabilities: Partial<CanvasSurfaceCapabilities>) {
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
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-comparison'));
  views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const native = nativePreviewHost(false);
  const host = { ...native.host, capabilities: { ...native.host.capabilities, ...capabilities } };
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
    acceptsPreparation: () => false,
    nativeSurface: () => host,
    connected: () => true,
    open: () => {},
    report: (error) => errors.push(error),
  });
  surface.port.updateSurface(address, previewGeometry);
  retirements.push(async () => {
    const retired = surface.dispose();
    for (const [index, record] of native.pending.entries())
      if (!record.settled)
        native.respond(
          index,
          native.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await retired;
  });
  const [a, , c] = catalog.entries.map((entry) => entry.items[0]!);
  return {
    ...native,
    surface,
    errors,
    a: a!,
    c: c!,
    /** Acknowledge every outstanding native ticket, answering camera requests with state. */
    async drain(viewport = finalViewport): Promise<void> {
      for (let guard = 0; guard < 16; guard++) {
        await settle();
        const index = native.pending.findIndex((record) => !record.settled);
        if (index < 0) return;
        const call = native.calls[index]!;
        native.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? viewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
    async dispatch(command: CanvasCommand): Promise<string> {
      const outcome = surface.port.dispatch(
        surface.port.createRequest(command, surface.port.read().revision),
      );
      return (await outcome).status;
    },
  };
}

test('a host without comparison keeps the exact camera-only command set and no B state', async () => {
  const f = fixture({ viewportCommands: ['fit', 'native', 'zoom', 'pan', 'wipe'] });
  await f.drain();
  expect(f.surface.port.capabilities.commands).toEqual([
    'reveal',
    'select',
    'select-source',
    'batch',
    'follow',
    'open-media',
    'fit',
    'native',
    'zoom',
    'pan',
  ]);
  const snapshot = f.surface.port.read();
  expect(snapshot.comparisonAvailable).toBe(false);
  expect(snapshot.view.comparison).toBeNull();
  expect(snapshot.secondary).toBeNull();
  expect(await f.dispatch({ type: 'comparison', enabled: true })).toBe('unsupported');
  await f.drain();
  expect(f.calls.some((call) => call.type === 'surface.select' && 'comparison' in call)).toBe(
    false,
  );
});

test('compares, places, swaps and rechooses B through the shared selection owner', async () => {
  const f = fixture(comparing);
  await f.drain();
  expect(f.surface.port.capabilities.commands).toEqual(
    expect.arrayContaining(['comparison', 'select-comparison', 'swap', 'wipe']),
  );
  expect(f.surface.port.read().comparisonAvailable).toBe(true);

  expect(await f.dispatch({ type: 'comparison', enabled: true })).toBe('applied');
  await f.drain(pairViewport);
  const selects = () => f.calls.filter((call) => call.type === 'surface.select');
  const wipes = () =>
    f.calls.flatMap((call) =>
      call.type === 'surface.viewport.request' && call.command.type === 'wipe'
        ? [call.command]
        : [],
    );
  expect(selects().at(-1)).toMatchObject({
    itemId: f.a.id,
    comparison: { itemId: f.c.id, file: f.c.file },
  });
  expect(wipes().at(-1)).toEqual({ type: 'wipe', orientation: 'vertical', split: 0.5 });
  let snapshot = f.surface.port.read();
  expect(snapshot.secondary?.id).toBe(f.c.id);
  expect(snapshot.secondaryResource?.state).toBe('ready');
  expect(snapshot.zoom?.secondaryDimensions).toEqual({ width: 512, height: 384 });
  expect(snapshot.zoom?.secondaryPhysicalScale).toEqual({ x: 2, y: 2 });

  expect(await f.dispatch({ type: 'wipe', orientation: 'horizontal', split: 0.25 })).toBe(
    'applied',
  );
  await f.drain(pairViewport);
  expect(wipes().at(-1)).toEqual({ type: 'wipe', orientation: 'horizontal', split: 0.25 });
  expect(f.surface.port.read().view.comparison).toMatchObject({ split: 0.25 });

  const selected = selects().length;
  expect(await f.dispatch({ type: 'swap' })).toBe('applied');
  await f.drain(pairViewport);
  expect(selects()).toHaveLength(selected + 1);
  expect(selects().at(-1)).toMatchObject({ itemId: f.c.id, comparison: { itemId: f.a.id } });
  expect(wipes().at(-1)).toEqual({ type: 'wipe', orientation: 'horizontal', split: 0.25 });

  const b = f.surface.port.read().sources[1]!;
  expect(
    await f.dispatch({
      type: 'select-comparison',
      selection: { sourceId: b.id, batchIndex: 0 },
    }),
  ).toBe('applied');
  await f.drain(pairViewport);
  expect(selects()).toHaveLength(selected + 2);
  snapshot = f.surface.port.read();
  expect(selects().at(-1)).toMatchObject({
    itemId: f.c.id,
    comparison: { itemId: snapshot.secondary?.id },
  });
  expect(snapshot.secondary?.provenance.sourceId).toBe(b.id);

  expect(await f.dispatch({ type: 'comparison', enabled: false })).toBe('applied');
  await f.drain();
  expect(selects().at(-1)).not.toHaveProperty('comparison');
  snapshot = f.surface.port.read();
  expect(snapshot.view.comparison).toBeNull();
  expect(snapshot.secondary).toBeNull();
  expect(snapshot.zoom?.secondaryDimensions).toBeNull();
  expect(f.errors).toEqual([]);
});
