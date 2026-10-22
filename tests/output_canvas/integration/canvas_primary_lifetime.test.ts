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
/** Prove primary tab/renderer visibility transitions release real composed image resources before replacement. */

import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { BrowserCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasAttachment.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { BrowserCanvasPlatformFixture } from '../support/BrowserCanvasPlatformFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

/** Replace only platform scheduling, geometry, and decoding; retain actual application and attachment ownership. */
function fixture() {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: 800,
    height: 600,
    right: 800,
    bottom: 600,
    toJSON: () => ({}),
  });
  let hidden = false;
  jest.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden);
  jest
    .spyOn(document, 'visibilityState', 'get')
    .mockImplementation(() => (hidden ? 'hidden' : 'visible'));
  const a = canvasCatalogFixture(1, 0, undefined, 'a');
  const b = canvasCatalogFixture(1, 0, undefined, 'b');
  const sessions = new WorkflowSessionStore();
  const results = new WorkflowResultStore();
  const contexts = [a.result.context, b.result.context];
  results.synchronize(contexts);
  sessions.synchronize(contexts, contexts[0]!.sessionId);
  const inventory = new CanvasSourceInventory();
  for (const catalog of [a, b]) {
    inventory.replace(
      catalog.result.context,
      catalog.entries.map((entry) => entry.source),
    );
    for (const entry of catalog.entries)
      results.restore(
        catalog.result.context,
        entry.source,
        {
          images: entry.items.map((item) => ({
            file: item.file,
            dimensions: item.dimensions,
            mediaKind: item.mediaKind,
          })),
          videos: [],
          models: [],
          text: [],
        },
        'fixture',
      );
  }
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const inspections = new CanvasInspectionStore();
  const allowances = new CanvasImageAllowances();
  const coordinator = new CanvasSessionCoordinator(sessions, results, inventory, views, selection, {
    subscribe: () => () => undefined,
  });
  coordinator.install();
  const root = document.createElement('div');
  document.body.append(root);
  const frames = new CanvasFrameFixture();
  const clock = new WorkflowTestClock();
  const loaders: BrowserCanvasPlatformFixture[] = [];
  const addresses: Parameters<CanvasViewRegistry['accepts']>[0][] = [];
  let renderer = 'vue';
  let available = true;
  let fail = false;
  const report = jest.fn<(error: unknown) => void>();
  const primary = new CanvasPrimarySurface({
    document,
    views,
    coordinator,
    window: windowId('primary'),
    element: () => (available ? root : null),
    renderer: () => renderer,
    report,
    mount: (element, address, _catalog, accepts) => {
      expect(loaders.filter((loader) => !loader.disposed)).toHaveLength(0);
      addresses.push(address);
      if (fail) throw new Error('Mount failed.');
      const loader = new BrowserCanvasPlatformFixture();
      loaders.push(loader);
      return new BrowserCanvasAttachment(element, {
        address,
        views,
        selection,
        inspections,
        allowances,
        clock,
        frames,
        loader,
        logger: { error: report },
        accepts,
        connected: () => true,
        catalog: () => coordinator.catalog(address.viewId),
        url: (item) => `${document.location.origin}/view?id=${item.id}`,
        open: () => undefined,
        disposeLoader: () => loader.dispose(),
        report,
      });
    },
  });
  primary.setup();
  return {
    primary,
    coordinator,
    views,
    selection,
    contexts,
    sessions,
    results,
    loaders,
    addresses,
    frames,
    report,
    activate: (index: number) => sessions.synchronize(contexts, contexts[index]!.sessionId),
    hidden: (value: boolean) => {
      hidden = value;
      document.dispatchEvent(new Event('visibilitychange'));
    },
    available: (value: boolean) => {
      available = value;
      primary.reconcile();
    },
    renderer: (value: string) => {
      renderer = value;
      primary.reconcile();
    },
    fail: (value: boolean) => {
      fail = value;
    },
    dispose: () => {
      primary.dispose();
      coordinator.dispose();
      root.remove();
      jest.restoreAllMocks();
    },
  };
}

test('retains the same image attachment through history restoration and releases it on failure', async () => {
  const f = fixture();
  try {
    const address = f.primary.diagnostics().address;
    const view = f.coordinator.active();
    const token = f.sessions.beginLoad('history');
    expect(f.sessions.read().active).toBeNull();
    expect(f.coordinator.active()).toBe(view);
    expect(f.primary.diagnostics().address).toBe(address);
    expect(f.loaders).toHaveLength(1);
    expect(f.loaders[0]?.disposed).toBe(false);
    f.sessions.completeLoad(token, true);
    expect(f.primary.diagnostics().address).toBe(address);
    const failed = f.sessions.beginLoad('history');
    f.sessions.completeLoad(failed, false);
    expect(f.primary.diagnostics().address).toBeNull();
    expect(f.loaders[0]?.disposed).toBe(true);
  } finally {
    f.dispose();
  }
});

test('bounds ownership over 100 pending tab switches and preserves workflow Manual intent on return', async () => {
  const f = fixture();
  const first = f.coordinator.active()!;
  f.selection.manual(first);
  for (let index = 0; index < 100; index++) {
    f.activate((index + 1) % 2);
    expect(f.loaders.filter((loader) => !loader.disposed)).toHaveLength(1);
    expect(f.addresses.slice(0, -1).every((address) => !f.views.accepts(address))).toBe(true);
  }
  expect(f.coordinator.active()).toBe(first);
  expect(f.views.read(first).follow).toBe('manual');
  for (const loader of f.loaders.slice(0, -1)) {
    await loader.ready(0);
    expect(loader.requests.every((request) => request.released)).toBe(true);
  }
  f.dispose();
  expect(f.loaders.every((loader) => loader.disposed)).toBe(true);
  expect(f.frames.pending).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
});

test('releases hidden, suspended, renderer-replaced, and closed-workflow pixels without retiring other views', () => {
  const f = fixture();
  const first = f.coordinator.active()!;
  const independent = f.views.create(f.views.read(first).sessionId);
  f.hidden(true);
  expect(f.loaders.every((loader) => loader.disposed)).toBe(true);
  f.hidden(false);
  f.renderer('litegraph');
  expect(f.loaders).toHaveLength(3);
  f.available(false);
  expect(f.loaders.every((loader) => loader.disposed)).toBe(true);
  f.available(true);
  const token = f.sessions.beginLoad();
  expect(f.loaders.every((loader) => loader.disposed)).toBe(true);
  f.results.synchronize([{ ...f.contexts[0]!, generation: 1 }, f.contexts[1]!]);
  f.sessions.completeLoad(token, true);
  expect(f.loaders.filter((loader) => !loader.disposed)).toHaveLength(1);
  f.sessions.synchronize([f.contexts[1]!], f.contexts[1]!.sessionId);
  expect(() => f.views.read(first)).toThrow('unavailable');
  expect(f.views.read(independent.viewId)).toBe(independent);
  f.dispose();
  expect(f.frames.pending).toBe(0);
});

test('reports failed mounting and revokes the committed lease before a later explicit recovery', () => {
  const f = fixture();
  f.available(false);
  f.fail(true);
  f.available(true);
  expect(f.report).toHaveBeenCalledTimes(1);
  expect(f.views.accepts(f.addresses.at(-1)!)).toBe(false);
  f.fail(false);
  f.primary.reconcile();
  expect(f.loaders.filter((loader) => !loader.disposed)).toHaveLength(1);
  f.dispose();
});
