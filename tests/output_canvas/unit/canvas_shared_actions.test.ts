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
/** Exercise shared controls against actual navigation owners and an asynchronous surface. */

import { jest } from '@jest/globals';
import {
  applyCanvasAction,
  type CanvasActionSurface,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasActions.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasViewProjection } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewProjection.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import {
  CanvasSourceNavigation,
  type CanvasSourcePreparation,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceNavigation.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSurfacePort } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfacePort.js';
import type {
  CanvasCommand,
  CanvasCommandScope,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { canvasCommandScope, canvasSnapshot } from '../support/CanvasPortFixture.js';

/** Compose actual selection and source navigation with one asynchronous surface boundary. */
function fixture(resources?: CanvasSourcePreparation) {
  let next = 0;
  const views = new CanvasViewRegistry(() => String(++next));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const catalog = canvasCatalogFixture();
  const view = views.create(catalog.sessionId).viewId;
  selection.reconcile(view, catalog);
  const projection = new CanvasViewProjection(view, views, selection, new CanvasInspectionStore());
  const plan = () => projection.project(catalog);
  const sources = new CanvasSourceNavigation({
    resources: resources ?? { prepareSource: () => null, cancelPreparation: () => {} },
    selection,
    read: () => ({ plan: plan(), catalog }),
    cells: () => [],
  });
  const fit = jest.fn<() => Promise<boolean>>().mockResolvedValue(true);
  const pan = jest.fn<(x: number, y: number) => Promise<boolean>>().mockResolvedValue(true);
  const surface: CanvasActionSurface = {
    fit,
    native: () => false,
    zoom: () => false,
    pan,
    retry: () => false,
    admitLargeImage: () => false,
    open: () => {},
    media: () => false,
  };
  const snapshot = canvasSnapshot('actions');
  const scope = canvasCommandScope({
    schema: 1,
    address: snapshot.address,
    requestId: 1,
    expectedRevision: snapshot.revision,
    command: { type: 'fit' },
  });
  const owners = () => ({
    plan: plan(),
    catalog,
    selection,
    sources,
    surface,
    scope,
    snapshot: { ...snapshot, resource: { ...snapshot.resource, state: 'ready' as const } },
  });
  return { views, selection, catalog, view, fit, pan, owners };
}

test('routes source selection and comparison through the existing view while awaiting surface commands', async () => {
  const f = fixture();
  const selected = { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 };
  await expect(
    applyCanvasAction({ type: 'select-source', selection: selected }, f.owners()),
  ).resolves.toBe(true);
  expect(f.views.read(f.view).selection).toEqual(selected);
  await expect(
    applyCanvasAction({ type: 'pan', x: 0.25, y: -0.5, units: 'viewport' }, f.owners()),
  ).resolves.toBe(true);
  expect(f.pan).toHaveBeenCalledWith(
    0.25,
    -0.5,
    expect.objectContaining({ request: f.owners().scope.request }),
  );
  await expect(applyCanvasAction({ type: 'comparison', enabled: true }, f.owners())).resolves.toBe(
    true,
  );
  expect(f.views.read(f.view).comparison).not.toBeNull();
});

test('a new camera action cancels a pending source handoff before late readiness', async () => {
  let resolve!: (ready: boolean) => void;
  const ready = new Promise<boolean>((done) => {
    resolve = done;
  });
  const cancelled = jest.fn();
  const f = fixture({
    prepareSource: () => ({ ready, unavailable: false, commit: (apply) => apply() }),
    cancelPreparation: cancelled,
  });
  const before = f.views.read(f.view).selection;
  const selection = { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 };
  const pending = applyCanvasAction({ type: 'select-source', selection }, f.owners());
  await expect(applyCanvasAction({ type: 'fit' }, f.owners())).resolves.toBe(true);
  resolve(true);
  await expect(pending).resolves.toBe(false);
  expect(cancelled).toHaveBeenCalled();
  expect(f.fit).toHaveBeenCalledTimes(1);
  expect(f.views.read(f.view).selection).toEqual(before);
});

test('does not mutate the surface when source retirement reenters with newer authority', async () => {
  let current = true;
  const f = fixture({
    prepareSource: () => null,
    cancelPreparation: () => {
      current = false;
    },
  });
  const owners = f.owners();
  await expect(
    applyCanvasAction(
      { type: 'fit' },
      { ...owners, scope: { ...owners.scope, current: () => current } },
    ),
  ).resolves.toBe(false);
  expect(f.fit).not.toHaveBeenCalled();
});

test('passes the exact admitted request to the preparing surface and cancels it on a newer control', async () => {
  let resolve!: (ready: boolean) => void;
  const ready = new Promise<boolean>((done) => {
    resolve = done;
  });
  let admitted!: CanvasCommandScope;
  const f = fixture({
    prepareSource: (_selection, _catalog, _cells, scope) => {
      admitted = scope;
      return { ready, unavailable: false, commit: (apply) => apply() };
    },
    cancelPreparation: () => {},
  });
  const initial = f.owners().snapshot;
  const port = new CanvasSurfacePort(initial, {
    origin: 'http://localhost',
    capabilities: { schema: 1, commands: ['select-source', 'fit'] },
    accepts: () => true,
    apply: (command, scope) => applyCanvasAction(command, { ...f.owners(), scope }),
    geometry: () => {},
    release: () => {},
    report: (error) => {
      throw error;
    },
  });
  const selection = { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 };
  const request = port.createRequest({ type: 'select-source', selection }, initial.revision);
  const pending = port.dispatch(request);
  expect(admitted.request).toEqual(request);
  const fit = port.createRequest({ type: 'fit' }, initial.revision);
  expect((await port.dispatch(fit)).status).toBe('applied');
  expect(admitted.signal.aborted).toBe(true);
  resolve(true);
  expect((await pending).status).toBe('stale');
  expect(f.views.read(f.view).selection).not.toEqual(selection);
  port.dispose();
});

test('comparison is refused while the current surface is not ready or shows a preview', async () => {
  const f = fixture();
  const owners = f.owners();
  await expect(
    applyCanvasAction(
      { type: 'comparison', enabled: true },
      {
        ...owners,
        snapshot: { ...owners.snapshot, preview: true },
      },
    ),
  ).resolves.toBe(false);
  await expect(
    applyCanvasAction(
      { type: 'comparison', enabled: true },
      {
        ...owners,
        snapshot: {
          ...owners.snapshot,
          resource: { ...owners.snapshot.resource, state: 'loading' },
        },
      },
    ),
  ).resolves.toBe(false);
  expect(f.views.read(f.view).comparison).toBeNull();
});

test('rejects all preview camera commands before the surface while finals remain navigable during inference', async () => {
  const f = fixture();
  const owners = f.owners();
  const commands: readonly CanvasCommand[] = [
    { type: 'fit' },
    { type: 'native' },
    { type: 'zoom', factor: 2 },
    { type: 'pan', x: 0.1, y: 0, units: 'viewport' },
  ];
  const changed = jest.fn<() => boolean>().mockReturnValue(true);
  const cancelled = jest.spyOn(owners.sources, 'cancel');
  const surface = { ...owners.surface, fit: changed, native: changed, zoom: changed, pan: changed };
  for (const command of commands)
    await expect(
      applyCanvasAction(command, {
        ...owners,
        surface,
        snapshot: { ...owners.snapshot, preview: true, runStatus: 'running' },
      }),
    ).resolves.toBe(false);
  expect(changed).not.toHaveBeenCalled();
  expect(cancelled).not.toHaveBeenCalled();
  for (const command of commands)
    await expect(
      applyCanvasAction(command, {
        ...owners,
        surface,
        snapshot: { ...owners.snapshot, preview: false, runStatus: 'running' },
      }),
    ).resolves.toBe(true);
  expect(changed).toHaveBeenCalledTimes(4);
});
