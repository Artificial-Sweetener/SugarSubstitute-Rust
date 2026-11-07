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
/** Qualify native-image/media handoff with real presentation owners and controlled native acknowledgements. */
import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { BrowserCanvasMediaDetail } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaDetail.js';
import { BrowserCanvasMediaSessions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaSessions.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import {
  OutputPresenterSelection,
  canvasRequiresBrowserMedia,
} from '../../../frontend/comfyui/ui/output_canvas/application/OutputPresenterSelection.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  mixedMediaFixture,
  CanvasModelRuntimeFixture,
} from '../support/CanvasMixedMediaFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  settleNativePreview,
} from '../support/NativeCanvasPreviewFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Compose actual selection, native and browser-media owners; fake only OS acknowledgements and geometry. */
function fixture(webp = false) {
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  jest.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
  jest.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {});
  const mixed = mixedMediaFixture();
  if (webp) {
    const item = mixed.items[2];
    if (!item) throw new Error('Missing mixed source slot.');
    mixed.items[2] = {
      ...item,
      mediaKind: 'image',
      encodedMime: 'image/webp',
      file: { ...item.file, filename: 'retained.webp' },
    };
  }
  const context = mixed.catalog.result.context;
  const source = mixed.catalog.entries[0]!.source;
  const sessions = new WorkflowSessionStore();
  const results = new WorkflowResultStore();
  results.synchronize([context]);
  sessions.synchronize([context], context.sessionId);
  results.restore(
    context,
    source,
    { images: mixed.items, videos: [], models: [], text: [] },
    'mixed',
  );
  const inventory = new CanvasSourceInventory();
  inventory.replace(context, [source]);
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const inspections = new CanvasInspectionStore();
  const coordinator = new CanvasSessionCoordinator(sessions, results, inventory, views, selection, {
    subscribe: () => () => {},
  });
  coordinator.install();
  const view = coordinator.active();
  if (!view) throw new Error('Media workflow fixture was not committed.');
  const read = () => {
    const catalog = coordinator.catalog(view);
    if (!catalog) throw new Error('Media fixture catalog is unavailable.');
    return canvasDisplayPlan(
      views.read(view),
      catalog,
      selection.items(view, catalog),
      selection.preview(view, catalog),
    );
  };
  const choose = (batchIndex: number) => {
    const catalog = coordinator.catalog(view);
    if (!catalog) throw new Error('Media fixture catalog is unavailable.');
    selection.apply(view, catalog, {
      type: 'select',
      selection: { sourceId: source.id, batchIndex },
    });
  };
  choose(0);
  const root = document.createElement('section');
  document.body.append(root);
  const native = nativePreviewHost(false);
  const clock = new WorkflowTestClock();
  const errors: unknown[] = [];
  const media = new BrowserCanvasMediaSessions();
  let input = false;
  const presenter = new OutputPresenterSelection({
    requiresNative: () => true,
    surface: () => native.host,
    discovered: () => true,
    browserMedia: () =>
      !input && coordinator.active() !== null && canvasRequiresBrowserMedia(read()),
  });
  const mounted = { native: 0, browser: 0 };
  const primary = new CanvasPrimarySurface({
    document,
    views,
    coordinator,
    window: windowId('media'),
    clock,
    element: () => root,
    renderer: () => presenter.current(),
    retireBeforeMount: (previous, next) => (previous === 'browser') !== (next === 'browser'),
    report: (error) => errors.push(error),
    mount: (element, address, _catalog, accepts, acceptsPreparation) => {
      if (presenter.current() !== 'browser') {
        mounted.native++;
        return new NativeCanvasAttachment(element, {
          address,
          clock,
          views,
          selection,
          frames: new CanvasFrameFixture(),
          inspections,
          catalog: () => coordinator.catalog(view),
          accepts,
          acceptsPreparation,
          nativeSurface: () => native.host,
          connected: () => true,
          report: (error) => errors.push(error),
        });
      }
      mounted.browser++;
      const detail = new BrowserCanvasMediaDetail(element, {
        models: new CanvasModelRuntimeFixture(),
        sessions: media,
        clock,
        url: (item) => `${document.location.origin}/view?filename=${item.file.filename}`,
        changed: () => {},
        report: (error) => errors.push(error),
      });
      const refresh = () => detail.reconcile(read(), true);
      refresh();
      return {
        diagnostics: () => ({
          displayed: 0,
          pendingDecodes: 0,
          queuedInputs: 0,
          previewDrops: 0,
          estimatedBytes: 0,
          unknownLoads: 0,
          activeDeadlines: 0,
          renderFrames: 0,
        }),
        refresh,
        wants: () => false,
        receive: () => {},
        dispose: () => detail.dispose(),
      };
    },
  });
  primary.setup();
  const drain = async (release = true) => {
    for (let guard = 0; guard < 32; guard++) {
      await settleNativePreview();
      const index = native.pending.findIndex(
        (record, at) =>
          !record.settled && (release || native.calls[at]?.type !== 'surface.release'),
      );
      if (index < 0) return;
      native.respond(
        index,
        native.calls[index]?.type === 'surface.prepare' ? 'ready' : 'applied',
        native.calls[index]?.type === 'surface.viewport.request' ? finalViewport : undefined,
      );
    }
    throw new Error('Native media handoff fixture did not settle.');
  };
  return {
    root,
    native,
    mounted,
    primary,
    errors,
    choose,
    sessions,
    clock,
    drain,
    input(value: boolean) {
      input = value;
      primary.reconcile();
    },
    async dispose() {
      primary.dispose();
      await drain();
      coordinator.dispose();
      media.dispose();
      sessions.dispose();
      results.dispose();
      inspections.dispose();
      inventory.dispose();
      root.remove();
    },
  };
}

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('image to video waits for native release then returns to native image after retiring the real player', async () => {
  const f = fixture();
  try {
    await f.drain();
    expect(f.mounted).toEqual({ native: 1, browser: 0 });
    f.choose(1);
    await f.drain(false);
    expect(f.native.calls.some((call) => call.type === 'surface.release')).toBe(true);
    expect(f.mounted.browser).toBe(0);
    expect(f.root.querySelector('video')).toBeNull();
    await f.drain();
    const video = f.root.querySelector('video');
    expect(video).not.toBeNull();
    video?.dispatchEvent(new Event('loadeddata'));
    f.choose(0);
    await f.drain();
    expect(video?.hasAttribute('src')).toBe(false);
    expect(video?.pause).toHaveBeenCalled();
    expect(f.mounted).toEqual({ native: 2, browser: 1 });
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
});

test('Input keeps native ownership and closing while release is pending cannot revive a media player', async () => {
  const f = fixture();
  try {
    await f.drain();
    f.input(true);
    f.choose(3);
    await f.drain();
    expect(f.mounted.browser).toBe(0);
    f.input(false);
    await f.drain(false);
    f.sessions.synchronize([], null);
    await f.drain();
    expect(f.mounted.browser).toBe(0);
    expect(f.root.querySelector('img,video')).toBeNull();
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
  expect(f.clock.pending).toBe(0);
});

test('still WebP keeps native ownership through source changes', async () => {
  const f = fixture(true);
  try {
    await f.drain();
    f.choose(2);
    await f.drain();
    expect(f.mounted.browser).toBe(0);
    expect(f.native.calls.some((call) => call.type === 'surface.release')).toBe(false);
    expect(
      f.native.calls.some(
        (call) => call.type === 'surface.select' && call.file.filename === 'retained.webp',
      ),
    ).toBe(true);
    expect(f.root.querySelector('img,video')).toBeNull();
    f.choose(0);
    await f.drain();
    expect(f.mounted).toEqual({ native: 1, browser: 0 });
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
});
