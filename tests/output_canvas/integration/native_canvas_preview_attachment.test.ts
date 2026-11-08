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
/** Exercise shared controls and native encoded preview custody without browser pixel resources. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import type {
  CanvasRoutePort,
  CanvasRouteState,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasRoute.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture, canvasSources } from '../support/CanvasCatalogFixture.js';
import {
  NativePreviewBlob,
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  previewViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Mount real selection/view/overlay owners with deterministic external geometry and native replies. */
function fixture(firstPreview = false, staged = false, route?: CanvasRoutePort) {
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
  const original = canvasCatalogFixture(1, 0, canvasSources().slice(0, 1));
  const source = original.sources[0]!;
  const preview: WorkflowPreview = {
    sessionId: original.result.context.sessionId,
    generation: 0,
    promptId: 'prompt:preview',
    sourceId: source.id,
    runOrder: 2,
    sequence: 1,
    token: 'preview:1',
    dimensions: null,
  };
  const running = (empty: boolean): CanvasCatalogSnapshot =>
    canvasOutputCatalog(
      {
        ...original.result,
        outputs: empty ? [] : original.result.outputs,
        runOrder: 2,
        presentedRunOrder: 2,
        preview,
        runStatus: 'running',
      },
      original.sources,
    );
  let catalog = firstPreview ? running(true) : original;
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 2 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-preview'));
  if (!staged) views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const clock = new WorkflowTestClock();
  const host = nativePreviewHost();
  const errors: unknown[] = [];
  const image = jest.spyOn(globalThis, 'Image');
  const surface = new NativeCanvasAttachment(root, {
    address,
    clock,
    views,
    selection,
    frames: new CanvasFrameFixture(),
    inspections: new CanvasInspectionStore(),
    catalog: () => catalog,
    accepts: () => views.accepts(address),
    acceptsPreparation: () => staged && views.acceptsPreparation(address),
    nativeSurface: () => host.host,
    connected: () => true,
    ...(route ? { route } : {}),
    report: (error) => errors.push(error),
  });
  surface.port.updateSurface(address, previewGeometry);
  retirements.push(async () => {
    const retired = surface.dispose();
    for (const [index, pending] of host.pending.entries()) {
      if (!pending.settled)
        host.respond(
          index,
          host.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    }
    await retired;
  });
  return {
    ...host,
    surface,
    root,
    views,
    view,
    address,
    preview,
    clock,
    image,
    errors,
    running: () => {
      catalog = running(false);
      selection.reconcile(view, catalog);
      surface.refresh();
    },
    generation: () => {
      catalog = { ...catalog, generation: 1 };
      surface.refresh();
    },
  };
}

/** Acknowledge each initial native operation explicitly, including the authoritative final camera. */
async function commit(f: ReturnType<typeof fixture>, firstPreview: boolean): Promise<void> {
  f.ready('observing');
  await settle();
  for (let index = 0; index < (firstPreview ? 3 : 4); index++) {
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
  if (!firstPreview) {
    f.respond(4, 'applied', finalViewport);
    await settle();
  }
  await f.surface.ready;
}

test('shows first-ever native preview controls only after the actual accepted-pixels event', async () => {
  const f = fixture(true);
  await commit(f, true);
  expect(f.calls.map((call) => call.type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.commit',
  ]);
  expect(f.surface.wants(f.preview.sessionId, 0, f.preview.sourceId)).toBe(true);
  const blob = new NativePreviewBlob();
  f.surface.receive(f.preview, blob);
  f.respond(3);
  await settle();
  blob.release();
  await settle();
  f.respond(4);
  await settle();
  f.respond(5);
  await settle();
  expect(f.surface.port.read().preview).toBe(false);
  const begin = f.calls[3];
  if (begin?.type !== 'surface.preview.begin') throw new Error('Preview begin is missing.');
  f.emit({
    address: f.address,
    sequence: 1,
    token: f.preview.token,
    leaseRevision: begin.revision,
    status: 'presented',
    displayed: {
      type: 'preview',
      token: f.preview.token,
      leaseRevision: begin.revision,
      viewport: previewViewport,
    },
    message: '',
  });
  const snapshot = f.surface.port.read();
  expect(snapshot.preview).toBe(true);
  expect(snapshot.zoom?.mode).toBe('fit');
  expect(snapshot.zoom?.dimensions).toEqual({ width: 19, height: 17 });
  expect(snapshot.resource.state).toBe('ready');
  expect(f.image).not.toHaveBeenCalled();
  expect(f.root.querySelector('canvas,.sc-output-image-layer')).toBeNull();
  expect(f.errors).toEqual([]);
});

test('keeps the last accepted final interactive while current Run metadata has no final item', async () => {
  const f = fixture();
  await commit(f, false);
  expect(f.surface.port.read().zoom?.mode).toBe('native');
  f.running();
  await settle();
  const read = f.calls.at(-1);
  if (read?.type === 'surface.viewport.request' && read.command.type === 'read') {
    f.respond(f.calls.length - 1, 'applied', finalViewport);
    await settle();
  }
  // Catalog handoff deliberately has no current final; the native owner retains actual final custody.
  expect(f.surface.port.read().selected).toBeNull();
  expect(f.surface.port.read().preview).toBe(false);
  expect(f.surface.port.read().zoom?.mode).toBe('native');
  expect(f.surface.wants(f.preview.sessionId, 0, f.preview.sourceId)).toBe(true);
  const operation = f.surface.port.dispatch(
    f.surface.port.createRequest({ type: 'native' }, f.surface.port.read().revision),
  );
  const command = f.calls.at(-1);
  expect(command).toMatchObject({ type: 'surface.viewport.request', command: { type: 'native' } });
  f.respond(f.calls.length - 1, 'applied', finalViewport);
  expect((await operation).status).toBe('applied');
  expect(f.surface.port.read().preview).toBe(false);
  expect(f.image).not.toHaveBeenCalled();
});

test('refuses sibling preparation and old workflow-generation preview bytes before any Blob read', async () => {
  const f = fixture(true, true);
  f.ready('observing');
  await settle();
  expect(f.surface.wants(f.preview.sessionId, 0, f.preview.sourceId)).toBe(false);
  const blob = new NativePreviewBlob();
  f.surface.receive(f.preview, blob);
  expect(blob.reads).toHaveLength(0);
  expect(f.calls.some((call) => call.type.startsWith('surface.preview.'))).toBe(false);
  f.generation();
  expect(f.surface.wants(f.preview.sessionId, 1, f.preview.sourceId)).toBe(false);
});

test.each([
  { key: '+', type: 'zoom' },
  { key: 'ArrowRight', type: 'pan' },
] as const)(
  'Input admits the $key key after withdrawing an Output live preview',
  async ({ key, type }) => {
    let state: CanvasRouteState = {
      route: 'output',
      inputAvailable: true,
      labels: { input: 'Input', output: 'Output', group: 'Canvas route' },
    };
    const listeners = new Set<() => void>();
    const route: CanvasRoutePort = {
      read: () => state,
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
      choose: (value) => {
        state = { ...state, route: value };
        for (const listener of listeners) listener();
      },
      subscribeFocus: () => () => {},
    };
    const f = fixture(true, false, route);
    await commit(f, true);
    const blob = new NativePreviewBlob();
    f.surface.receive(f.preview, blob);
    f.respond(3);
    await settle();
    blob.release();
    await settle();
    f.respond(4);
    await settle();
    f.respond(5);
    await settle();
    const begin = f.calls[3];
    if (begin?.type !== 'surface.preview.begin') throw new Error('Preview begin is missing.');
    f.emit({
      address: f.address,
      sequence: 1,
      token: f.preview.token,
      leaseRevision: begin.revision,
      status: 'presented',
      displayed: {
        type: 'preview',
        token: f.preview.token,
        leaseRevision: begin.revision,
        viewport: previewViewport,
      },
      message: '',
    });
    const press = () =>
      f.surface.viewport.dispatchEvent(
        new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
      );
    const offset = f.calls.length;
    expect(f.surface.port.read().preview).toBe(true);
    press();
    expect(f.calls).toHaveLength(offset);
    route.choose('input');
    await settle();
    expect(f.calls[offset]).toMatchObject({ type: 'surface.preview.withdraw' });
    expect(f.surface.port.read().preview).toBe(false);
    expect(f.surface.wants(f.preview.sessionId, 0, f.preview.sourceId)).toBe(false);
    const dispatch = jest.spyOn(f.surface.port, 'dispatch');
    press();
    expect(dispatch).toHaveBeenCalledTimes(1);
    // The preview withdrawal must finish before the queued Input camera command reaches the host.
    expect(f.calls).toHaveLength(offset + 1);
    f.respond(offset);
    await settle();
    expect(f.calls[offset + 1]).toMatchObject({
      type: 'surface.viewport.request',
      command: { type },
    });
    f.respond(offset + 1, 'applied', finalViewport);
    await settle();
    expect(f.surface.port.read().zoom?.mode).toBe('native');
    expect(f.calls.slice(offset).map((call) => call.type)).toEqual([
      'surface.preview.withdraw',
      'surface.viewport.request',
    ]);
    expect(f.errors).toEqual([]);
  },
);
