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
/** Present overview grids through the native revision stream owned by the presentation. */
import { afterEach, expect, test } from '@jest/globals';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import {
  NativeCanvasPresentation,
  type NativeCanvasPresentationStatus,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceOverviewCell,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { connectCanvasSurface } from '../../editor_integrations/support/canvas_surface.js';
import { canvasNavigationScope, canvasSnapshot } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  NativePreviewBlob,
  finalViewport,
  nativePreviewHost,
  previewGeometry as geometry,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const overviewing: Partial<CanvasSurfaceCapabilities> = { overview: true };
const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
});

/** Describe painted cells in one row, offset so successive layouts stay distinguishable. */
function cells(count: number, offset = 0): CanvasSurfaceOverviewCell[] {
  return Array.from({ length: count }, (_, index) => ({
    itemId: `result:cell-${index}`,
    file: { filename: `ComfyUI_${index}.png`, subfolder: '', type: 'output' },
    rect: { x: offset + index * 120, y: 10, width: 110, height: 80 },
  }));
}

/** Compose the real presentation owner over a held native host. */
function fixture(
  capabilities: Partial<CanvasSurfaceCapabilities> = overviewing,
  preview = false,
  transport?: CanvasClient,
) {
  const native = nativePreviewHost(preview);
  const host = { ...native.host, capabilities: { ...native.host.capabilities, ...capabilities } };
  const catalog = canvasCatalogFixture();
  const address = canvasSnapshot('overview').address;
  const statuses: NativeCanvasPresentationStatus[] = [];
  const viewports: (CanvasSurfaceViewportState | null)[] = [];
  const previews: boolean[] = [];
  const visible: boolean[] = [];
  const errors: unknown[] = [];
  const presentation = new NativeCanvasPresentation({
    address,
    clock: new WorkflowTestClock(),
    host: () => transport?.surface ?? host,
    accepts: () => true,
    acceptsPreparation: () => false,
    prepared: () => {},
    visible: (value) => visible.push(value),
    status: (value) => statuses.push(value),
    viewport: (value) => viewports.push(value),
    preview: (value) => previews.push(value),
    report: (error) => errors.push(error),
  });
  retirements.push(async () => {
    const retired = presentation.dispose();
    for (const [index, record] of native.pending.entries())
      if (!record.settled)
        native.respond(
          index,
          native.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await retired;
  });
  const item = catalog.entries[0]!.items[0]!;
  return { ...native, address, presentation, statuses, viewports, previews, visible, errors, item };
}

/** Acknowledge preparation, placement, the overview and commit of a first grid. */
async function presentGrid(f: ReturnType<typeof fixture>, grid = cells(2)): Promise<void> {
  f.presentation.update(geometry, null, null, null, grid);
  for (let index = 0; index < 4; index++) {
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
}

test('presents the grid as one overview selection and serves no camera', async () => {
  const f = fixture();
  const grid = cells(3);
  await presentGrid(f, grid);
  expect(f.calls.map(({ type }) => type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.overview',
    'surface.commit',
  ]);
  expect(f.calls[2]).toEqual({
    type: 'surface.overview',
    address: f.address,
    revision: 3,
    cells: grid,
  });
  expect(f.statuses.at(-1)).toEqual({ state: 'ready', message: '' });
  expect(f.visible.at(-1)).toBe(true);
  expect(f.viewports.at(-1)).toBeNull();
  expect(await f.presentation.viewport({ type: 'zoom', factor: 2 }, canvasNavigationScope())).toBe(
    false,
  );
  await settle();
  expect(f.calls).toHaveLength(4);
  expect(f.presentation.wants()).toBe(false);
  expect(f.errors).toEqual([]);
});

test('coalesces layout changes to one overview in flight and only the newest pending', async () => {
  const f = fixture();
  await presentGrid(f);
  f.presentation.update(geometry, null, null, null, cells(2, 10));
  await settle();
  expect(f.calls[4]).toMatchObject({ type: 'surface.overview', cells: cells(2, 10) });
  f.presentation.update(geometry, null, null, null, cells(2, 20));
  f.presentation.update(geometry, null, null, null, cells(3, 30));
  await settle();
  expect(f.calls).toHaveLength(5);
  expect(f.pending[4]!.cancelled).toBe(false);
  f.respond(4);
  await settle();
  expect(f.calls).toHaveLength(6);
  expect(f.calls[5]).toEqual({
    type: 'surface.overview',
    address: f.address,
    revision: 6,
    cells: cells(3, 30),
  });
  f.respond(5);
  await settle();
  f.presentation.update(geometry, null, null, null, cells(3, 30));
  await settle();
  expect(f.calls).toHaveLength(6);
  expect(f.statuses.at(-1)?.state).toBe('ready');
});

test('leaving the overview selects one result and reads its camera, and re-entry resends the grid', async () => {
  const f = fixture();
  await presentGrid(f);
  f.presentation.update(geometry, f.item);
  await settle();
  expect(f.calls[4]).toEqual({
    type: 'surface.select',
    address: f.address,
    revision: 5,
    itemId: f.item.id,
    file: f.item.file,
  });
  f.respond(4);
  await settle();
  expect(f.calls[5]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  f.respond(5, 'applied', finalViewport);
  await settle();
  expect(f.viewports.at(-1)).toEqual(finalViewport);
  f.presentation.update(geometry, null, null, null, cells(2));
  await settle();
  expect(f.calls[6]).toMatchObject({ type: 'surface.overview', cells: cells(2) });
  expect(f.viewports.at(-1)).toBeNull();
});

test('an overview without paintable cells hides the surface instead of a retained final', async () => {
  const f = fixture();
  f.presentation.update(geometry, f.item);
  for (let index = 0; index < 4; index++) {
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
  f.respond(4, 'applied', finalViewport);
  await settle();
  f.presentation.update(geometry, null, null, null, []);
  await settle();
  expect(f.calls[5]).toMatchObject({
    type: 'surface.geometry',
    geometry: { visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } },
  });
  expect(f.visible.at(-1)).toBe(false);
  f.respond(5);
  await settle();
  expect(f.calls).toHaveLength(6);
  expect(f.statuses.at(-1)?.state).toBe('empty');
});

test('a refused overview reports the host outcome instead of retrying it', async () => {
  const f = fixture();
  f.presentation.update(geometry, null, null, null, cells(2));
  f.respond(0, 'ready');
  await settle();
  f.respond(1);
  await settle();
  f.respond(2, 'error');
  await settle();
  expect(f.statuses.at(-1)?.state).toBe('error');
  expect(f.errors).toHaveLength(1);
  expect(f.calls).toHaveLength(3);
});

test('a host without overview support never receives a grid', async () => {
  const f = fixture({ overview: false });
  f.presentation.update(geometry, null, null, null, cells(2));
  for (let turn = 0; turn < 4; turn++) {
    const index = f.pending.findIndex((record) => !record.settled);
    if (index >= 0)
      f.respond(index, f.calls[index]?.type === 'surface.prepare' ? 'ready' : 'applied');
    await settle();
  }
  expect(f.calls.some(({ type }) => type === 'surface.overview')).toBe(false);
});

test('accepts an overview display only for the presented selection revision', async () => {
  const f = fixture({}, true);
  const catalog = canvasCatalogFixture();
  const preview: WorkflowPreview = {
    sessionId: catalog.result.context.sessionId,
    generation: catalog.generation,
    promptId: 'prompt:preview',
    sourceId: f.item.provenance.sourceId,
    runOrder: 1,
    sequence: 1,
    token: 'preview:1',
    dimensions: null,
  };
  f.presentation.update(geometry, f.item, preview);
  f.ready('observing');
  await settle();
  for (let index = 0; index < 4; index++) {
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
  f.respond(4, 'applied', finalViewport);
  await settle();
  const blob = new NativePreviewBlob();
  f.presentation.receive(preview, blob);
  f.respond(5);
  await settle();
  blob.release();
  await settle();
  f.respond(6);
  await settle();
  f.respond(7);
  await settle();
  const lease = f.calls[5];
  const selection = f.calls[2];
  if (lease?.type !== 'surface.preview.begin' || selection?.type !== 'surface.select')
    throw new Error('Missing preview lease.');
  const observe = (selectionRevision: number, sequence: number) =>
    f.emit({
      address: f.address,
      token: lease.preview.token,
      leaseRevision: lease.revision,
      sequence,
      status: 'cancelled',
      displayed: { type: 'overview', selectionRevision },
      message: '',
    });
  const states = f.statuses.length;
  observe(selection.revision + 40, 1);
  await settle();
  expect(f.statuses).toHaveLength(states);
  expect(f.viewports.at(-1)).toEqual(finalViewport);
  observe(selection.revision, 2);
  await settle();
  expect(f.previews.at(-1)).toBe(false);
  expect(f.viewports.at(-1)).toBeNull();
  expect(f.statuses.at(-1)).toEqual({ state: 'ready', message: '' });
  expect(f.errors).toEqual([]);
});

type SurfaceRequest = Extract<CanvasMessage, { readonly address: unknown }>;

/** Answer one exact surface request with a status-only reply. */
function statusReply(request: SurfaceRequest, status: 'ready' | 'applied' | 'cancelled') {
  return {
    protocol: request.protocol,
    version: request.version,
    session: request.session,
    requestId: request.requestId,
    type: 'surface.reply',
    address: request.address,
    revision: request.revision,
    status,
    message: '',
  } as const;
}

test('ignores foreign and post-cancellation overview replies on the real canvas transport', async () => {
  const sent: CanvasMessage[] = [];
  const reports: unknown[] = [];
  const client = new CanvasClient('native-overview-transport', {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: (error) => reports.push(error),
  });
  const capabilities = { ...nativePreviewHost(false).host.capabilities, overview: true };
  await connectCanvasSurface(client, sent, capabilities);
  const f = fixture(overviewing, false, client);
  const outstanding = (): SurfaceRequest => {
    const request = sent.at(-1);
    if (!request || !('address' in request)) throw new Error('Missing surface request.');
    return request;
  };
  f.presentation.update(geometry, null, null, null, cells(2));
  for (const status of ['ready', 'applied'] as const) {
    expect(client.receive(statusReply(outstanding(), status))).toBe(true);
    await settle();
  }
  const grid = outstanding();
  expect(grid).toMatchObject({ type: 'surface.overview', cells: cells(2) });
  const foreign = { ...grid, requestId: grid.requestId + 50 };
  expect(client.receive(statusReply(foreign, 'applied'))).toBe(false);
  await settle();
  expect(sent.at(-1)).toBe(grid);
  expect(f.statuses.at(-1)?.state).toBe('loading');
  expect(client.receive(statusReply(grid, 'applied'))).toBe(true);
  await settle();
  expect(client.receive(statusReply(outstanding(), 'applied'))).toBe(true);
  await settle();
  expect(f.statuses.at(-1)?.state).toBe('ready');

  f.presentation.update(geometry, null, null, null, cells(2, 40));
  await settle();
  const superseded = outstanding();
  expect(superseded).toMatchObject({ type: 'surface.overview', cells: cells(2, 40) });
  f.presentation.update(geometry, f.item);
  expect(sent.at(-1)).toMatchObject({ type: 'cancel', requestId: superseded.requestId });
  expect(client.receive(statusReply(superseded, 'cancelled'))).toBe(true);
  await settle();
  expect(client.receive(statusReply(superseded, 'applied'))).toBe(false);
  await settle();
  expect(outstanding()).toMatchObject({ type: 'surface.select', itemId: f.item.id });
  expect(reports).toEqual([]);
  expect(f.errors).toEqual([]);
  // Closing the session rejects the release it would otherwise acknowledge.
  retirements.splice(0);
  const retired = f.presentation.dispose();
  client.dispose();
  await expect(retired).rejects.toThrow('Canvas connection closed.');
});
