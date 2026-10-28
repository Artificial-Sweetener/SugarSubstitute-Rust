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
/** Exercise the composed browser surface and reusable Vue overlay against actual state owners. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { BrowserCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasAttachment.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
} from '../../../integrations/contracts/CanvasSurface.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { BrowserCanvasPlatformFixture } from '../support/BrowserCanvasPlatformFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { settleCanvasImages } from '../support/CanvasImageFixture.js';
import {
  mixedMediaFixture,
  CanvasModelRuntimeFixture,
} from '../support/CanvasMixedMediaFixture.js';

/** Allocate real owners and replace only DOM measurement and platform decoding. */
function fixture(brokenUrl = false) {
  const bounds = jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 800,
    height: 600,
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    toJSON: () => ({}),
  });
  let catalog: CanvasCatalogSnapshot = canvasCatalogFixture();
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  selection.apply(view, catalog, {
    type: 'select-source',
    selection: { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  const address = views.prepareAttachment(view, windowId('browser'));
  views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const clock = new WorkflowTestClock();
  const frames = new CanvasFrameFixture();
  const loader = new BrowserCanvasPlatformFixture();
  const inspections = new CanvasInspectionStore();
  const allowances = new CanvasImageAllowances();
  const errors: unknown[] = [];
  const options = {
    address,
    views,
    selection,
    inspections,
    allowances,
    clock,
    frames,
    loader,
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
    catalog: () => catalog,
    accepts: () => views.accepts(address) && catalog.generation === 0,
    connected: () => true,
    url: (item: { readonly id: string }) => {
      if (brokenUrl) throw new Error('File URL is unavailable.');
      return `${document.location.origin}/view?id=${item.id}`;
    },
    open: jest.fn(),
    disposeLoader: () => loader.dispose(),
    report: (error: unknown) => {
      errors.push(error);
    },
  };
  /** Submit a control intent from its current qualified port revision. */
  const command = async (surface: Pick<BrowserCanvasAttachment, 'port'>, intent: CanvasCommand) =>
    surface.port.dispatch(surface.port.createRequest(intent, surface.port.read().revision));
  /** Supply drawable client geometry at the native boundary in a layout-free DOM. */
  const drawable = (surface: NativeCanvasAttachment) =>
    surface.port.updateSurface(address, {
      coordinateSpace: 'client',
      units: 'css-pixels',
      origin: { x: 0, y: 0 },
      viewport: { width: 800, height: 600 },
      clip: { x: 0, y: 0, width: 800, height: 600 },
      physicalPixelsPerCssPixel: 1,
      visible: true,
      focused: false,
    });
  /** Change the authoritative catalog at a named result boundary. */
  const replace = (next: CanvasCatalogSnapshot) => {
    catalog = next;
  };
  return {
    bounds,
    catalog,
    views,
    view,
    selection,
    address,
    root,
    clock,
    frames,
    loader,
    inspections,
    allowances,
    errors,
    options,
    command,
    drawable,
    replace,
  };
}

describe('composed browser canvas attachment', () => {
  afterEach(() => {
    document.body.replaceChildren();
    jest.restoreAllMocks();
  });

  test('presents native outputs and tabs without allocating browser image resources', async () => {
    const f = fixture();
    document.getElementById('sugarcubes-output-browser-styles')?.remove();
    const calls: CanvasSurfaceCommand[] = [];
    const host: CanvasSurfaceHost = {
      execute: async (command) => {
        calls.push(command);
        return { status: command.type === 'surface.prepare' ? 'ready' : 'applied', message: '' };
      },
    };
    const surface = new NativeCanvasAttachment(f.root, {
      ...f.options,
      nativeSurface: () => host,
    });
    f.drawable(surface);
    try {
      for (let index = 0; index < 24; index++) await Promise.resolve();
      expect(calls.map((call) => call.type)).toEqual([
        'surface.prepare',
        'surface.geometry',
        'surface.select',
        'surface.commit',
      ]);
      expect(calls[2]).toMatchObject({
        itemId: f.catalog.entries[0]!.items[0]!.id,
        file: f.catalog.entries[0]!.items[0]!.file,
      });
      expect(surface.viewport.classList.contains('sc-output-native')).toBe(true);
      expect(f.root.querySelector('.sc-output-tabstrip')).not.toBeNull();
      expect(f.root.querySelector('.sc-output-image-layer')).toBeNull();
      expect(f.loader.requests).toHaveLength(0);
      expect(document.getElementById('sugarcubes-output-browser-styles')).toBeNull();
      expect(document.getElementById('sugarcubes-output-viewport-styles')).not.toBeNull();

      const second = f.catalog.entries[1]!;
      const selected = f.command(surface, {
        type: 'select-source',
        selection: { sourceId: second.source.id, batchIndex: 0 },
      });
      expect((await selected).status).toBe('applied');
      for (let index = 0; index < 24; index++) await Promise.resolve();
      expect(calls.at(-1)).toMatchObject({ type: 'surface.select', itemId: second.items[0]!.id });
      expect(surface.viewport.classList.contains('sc-output-native')).toBe(true);
      expect(f.root.querySelector('.sc-output-tabstrip')).not.toBeNull();
      expect(f.loader.requests).toHaveLength(0);
      expect(f.root.querySelector('.sc-output-image-layer')).toBeNull();
    } finally {
      surface.dispose();
    }
    expect(calls.at(-1)).toMatchObject({ type: 'surface.release', address: f.address });
    expect(f.errors).toEqual([]);
  });

  test('keeps browser image resources absent while the native host connects or declines', async () => {
    const f = fixture(true);
    let host: CanvasSurfaceHost | null = null;
    const surface = new NativeCanvasAttachment(f.root, { ...f.options, nativeSurface: () => host });
    f.drawable(surface);
    expect(surface.port.read().resource.state).toBe('unavailable');
    expect(surface.wants()).toBe(false);
    expect(f.loader.requests).toHaveLength(0);
    expect(f.root.querySelector('.sc-output-image-layer')).toBeNull();
    const calls: CanvasSurfaceCommand[] = [];
    host = {
      execute: async (command) => {
        calls.push(command);
        return {
          status: command.type === 'surface.prepare' ? 'unavailable' : 'applied',
          message: 'Surface unavailable.',
        };
      },
    };
    surface.refresh();
    for (let index = 0; index < 12; index++) await Promise.resolve();
    expect(calls.map((call) => call.type)).toEqual(['surface.prepare']);
    expect(surface.port.read().resource).toMatchObject({
      state: 'unavailable',
      message: 'Surface unavailable.',
    });
    expect(f.loader.requests).toHaveLength(0);
    expect(f.root.querySelector('.sc-output-image-layer')).toBeNull();
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('switches native batch outputs without invoking a browser image loader', async () => {
    const f = fixture();
    const entry = f.catalog.entries[0]!;
    const first = entry.items[0]!;
    const second: WorkflowOutputItem = {
      ...first,
      id: `${first.id}:second`,
      batchIndex: 1,
      file: { ...first.file, filename: 'second.png' },
    };
    f.replace({
      ...f.catalog,
      entries: [{ ...entry, items: [first, second] }, ...f.catalog.entries.slice(1)],
    });
    const calls: CanvasSurfaceCommand[] = [];
    const host: CanvasSurfaceHost = {
      execute: async (command) => {
        calls.push(command);
        return { status: command.type === 'surface.prepare' ? 'ready' : 'applied', message: '' };
      },
    };
    const surface = new NativeCanvasAttachment(f.root, { ...f.options, nativeSurface: () => host });
    f.drawable(surface);
    for (let index = 0; index < 24; index++) await Promise.resolve();
    expect((await f.command(surface, { type: 'batch', index: 1 })).status).toBe('applied');
    for (let index = 0; index < 24; index++) await Promise.resolve();
    expect(calls.at(-1)).toMatchObject({
      type: 'surface.select',
      itemId: second.id,
      file: second.file,
    });
    surface.viewport.dispatchEvent(new Event('pointerdown'));
    surface.viewport.dispatchEvent(new Event('wheel'));
    expect(surface.viewport.classList.contains('sc-output-native')).toBe(true);
    expect(f.loader.requests).toHaveLength(0);
    expect(f.root.querySelector('.sc-output-image-layer')).toBeNull();
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('a newer canvas control cancels a pending graph-thumbnail reveal', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    f.frames.advance(250);
    const before = f.views.read(f.view).selection;
    const item = f.catalog.entries[1]!.items[0]!;
    const pending = surface.reveal(canvasRevealTarget(item));
    const index = f.loader.requests.findIndex((request) => request.input.key === item.id);
    expect(index).toBeGreaterThanOrEqual(0);
    await f.command(surface, { type: 'fit' });
    expect(await pending).toBe(false);
    await f.loader.ready(index);
    expect(f.views.read(f.view).selection).toEqual(before);
    expect(f.loader.requests[index]!.released).toBe(true);
    surface.dispose();
  });

  test('releases outgoing image gestures before Comfy model controls receive pointer input', async () => {
    const f = fixture();
    const mixed = mixedMediaFixture();
    f.replace(mixed.catalog);
    const models = new CanvasModelRuntimeFixture();
    const surface = new BrowserCanvasAttachment(f.root, { ...f.options, models });
    await f.loader.ready(0);
    f.frames.advance(250);
    await f.command(surface, { type: 'batch', index: 2 });
    f.frames.advance(250);
    const model = f.root.querySelector('.sc-output-model-container')!;
    const pointer = new MouseEvent('pointerdown', {
      bubbles: true,
      cancelable: true,
      button: 0,
      clientX: 400,
      clientY: 300,
    });
    model.dispatchEvent(pointer);
    expect(pointer.defaultPrevented).toBe(false);
    expect(f.root.querySelectorAll('.sc-output-image')).toHaveLength(0);
    surface.dispose();
    models.requests[0]!.resolve(models.viewer());
    await Promise.resolve();
    f.root.remove();
  });

  test('renders model failures literally and retries through the shared output overlay', async () => {
    const f = fixture();
    const report = jest.spyOn(f.options, 'report');
    f.replace(mixedMediaFixture().catalog);
    const models = new CanvasModelRuntimeFixture();
    f.views.update(f.view, (state) => ({
      ...state,
      sourcePreview: false,
      selection: { sourceId: f.catalog.entries[0]!.source.id, batchIndex: 2 },
    }));
    const surface = new BrowserCanvasAttachment(f.root, { ...f.options, models });
    models.requests[0]!.reject(new Error('Model <b>unavailable</b>'));
    await settleCanvasImages();
    f.clock.advance(100);
    await nextTick();
    expect(f.root.textContent).toContain('Model <b>unavailable</b>');
    expect(f.root.querySelector('b')).toBeNull();
    expect(report).not.toHaveBeenCalled();
    const retry = [...f.root.querySelectorAll('button')].find(
      (button) => button.textContent === 'Retry output',
    )!;
    retry.click();
    await settleCanvasImages();
    expect(models.requests).toHaveLength(2);
    surface.dispose();
    models.requests[1]!.resolve(models.viewer());
    await settleCanvasImages();
  });

  test.each(
    (['image', 'overview'] as const).flatMap((mode) =>
      [0, 1, 2].map((batchIndex) => [mode, batchIndex] as const),
    ),
  )(
    'prepares the exact %s batch file at index %s and retires it before a changed run can expose old pixels',
    async (mode, batchIndex) => {
      const f = fixture();
      const catalog = {
        ...f.catalog,
        entries: f.catalog.entries.map((entry) => ({
          ...entry,
          items: [0, 1, 2].map(
            (batchIndex): WorkflowOutputItem => ({
              ...entry.items[0]!,
              id: `${entry.items[0]!.id}:${batchIndex}`,
              batchIndex,
            }),
          ),
        })),
      };
      f.replace(catalog);
      f.views.update(f.view, (state) => ({
        ...state,
        mode,
        selection: { sourceId: catalog.entries[0]!.source.id, batchIndex },
      }));
      const drag = jest
        .fn<
          (item: WorkflowOutputItem, signal: AbortSignal) => Promise<{ file: File; url: string }>
        >()
        .mockResolvedValue({
          file: new File(['exact file'], 'batch-2.png', { type: 'image/png' }),
          url: 'http://localhost/view?filename=batch-2.png',
        });
      let invalidate: (() => void) | undefined;
      const stop = jest.fn();
      const surface = new BrowserCanvasAttachment(f.root, {
        ...f.options,
        drag,
        transferChanged: (listener) => {
          invalidate = listener;
          return stop;
        },
      });
      const handles: HTMLImageElement[] = [];
      for (let index = 0; index < (mode === 'overview' ? 3 : 1); index++)
        handles.push(await f.loader.ready(index));
      f.frames.advance(2);
      f.clock.advance(100);
      const image = handles[mode === 'overview' ? batchIndex : 0]!;
      if (mode === 'image') expect(getComputedStyle(image).pointerEvents).toBe('auto');
      const hover = () =>
        image.dispatchEvent(
          new MouseEvent('pointerover', {
            bubbles: true,
            clientX: 400,
            clientY: 300,
          }),
        );
      hover();
      await Promise.resolve();
      await Promise.resolve();
      expect(drag.mock.calls[0]?.[0]).toBe(catalog.entries[0]!.items[batchIndex]);
      expect(image.draggable).toBe(true);
      invalidate?.();
      expect(image.draggable).toBe(false);
      hover();
      await Promise.resolve();
      await Promise.resolve();
      f.replace({
        ...catalog,
        entries: catalog.entries.map((entry) => ({
          ...entry,
          items: entry.items.map(
            (item): WorkflowOutputItem => ({ ...item, id: `result:next-run:${item.id}` }),
          ),
        })),
      });
      surface.refresh();
      expect(image.draggable).toBe(false);
      const before = drag.mock.calls.length;
      hover();
      expect(drag).toHaveBeenCalledTimes(before);
      surface.dispose();
      expect(stop).toHaveBeenCalledTimes(1);
      expect(f.errors).toEqual([]);
    },
  );

  test.each(['image', 'overview'] as const)(
    'commits a prepared source and its %s pixels together without a blank intermediate presentation',
    async (mode) => {
      const f = fixture();
      const catalog = {
        ...f.catalog,
        entries: f.catalog.entries.map((entry) => ({
          ...entry,
          items: [0, 1, 2].map(
            (batchIndex): WorkflowOutputItem => ({
              ...entry.items[0]!,
              id: `${entry.items[0]!.id}:${batchIndex}`,
              batchIndex,
              dimensions: { width: 1080, height: 1512 },
            }),
          ),
        })),
      };
      f.replace(catalog);
      const first = catalog.entries[0]!,
        second = catalog.entries[1]!;
      f.views.update(f.view, (state) => ({
        ...state,
        mode,
        selection: { sourceId: first.source.id, batchIndex: 1 },
      }));
      const surface = new BrowserCanvasAttachment(f.root, f.options);
      const count = mode === 'overview' ? 3 : 1;
      for (let index = 0; index < count; index++)
        await f.loader.ready(index, { width: 1080, height: 1512 });
      f.frames.advance(2);
      f.clock.advance(100);
      const oldImages = [...f.root.querySelectorAll('img')];
      const pending = f.command(surface, {
        type: 'select-source',
        selection: { sourceId: second.source.id, batchIndex: 1 },
      });
      expect(f.views.read(f.view).selection?.sourceId).toBe(first.source.id);
      expect([...f.root.querySelectorAll('img')]).toEqual(oldImages);
      for (let index = 0; index < count; index++) {
        if (index === count - 1) {
          expect(f.views.read(f.view).selection?.sourceId).toBe(first.source.id);
          expect([...f.root.querySelectorAll('img')]).toEqual(oldImages);
        }
        await f.loader.ready(count + index, { width: 1080, height: 1512 });
      }
      expect((await pending).status).toBe('applied');
      f.frames.advance(2);
      f.clock.advance(100);
      expect(f.views.read(f.view).selection).toEqual({ sourceId: second.source.id, batchIndex: 1 });
      expect(f.root.querySelectorAll('img')).toHaveLength(count);
      expect([...f.root.querySelectorAll('img')].every((image) => !oldImages.includes(image))).toBe(
        true,
      );
      expect(f.loader.requests).toHaveLength(count * 2);
      surface.dispose();
      expect(f.errors).toEqual([]);
    },
  );

  test('a superseding camera command cancels source preparation and releases late decoded pixels', async () => {
    const f = fixture(),
      surface = new BrowserCanvasAttachment(f.root, f.options);
    const old = await f.loader.ready(0);
    f.frames.advance(2);
    f.clock.advance(100);
    const pending = f.command(surface, {
      type: 'select-source',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    await f.command(surface, { type: 'fit' });
    expect((await pending).status).toBe('stale');
    const late = await f.loader.ready(1);
    expect(late.isConnected).toBe(false);
    expect(f.root.querySelector('img')).toBe(old);
    expect(f.views.read(f.view).selection?.sourceId).toBe(f.catalog.entries[0]!.source.id);
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('target metadata supersession cannot commit prepared obsolete pixels', async () => {
    const f = fixture(),
      surface = new BrowserCanvasAttachment(f.root, f.options);
    const old = await f.loader.ready(0);
    f.frames.advance(2);
    f.clock.advance(100);
    const pending = f.command(surface, {
      type: 'select-source',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    f.replace({
      ...f.catalog,
      entries: f.catalog.entries.map((entry, index) =>
        index === 1
          ? {
              ...entry,
              items: entry.items.map(
                (item): WorkflowOutputItem => ({ ...item, id: 'result:revised-target' }),
              ),
            }
          : entry,
      ),
    });
    surface.refresh();
    const obsolete = await f.loader.ready(1);
    expect((await pending).status).toBe('unsupported');
    expect(obsolete.isConnected).toBe(false);
    expect(f.root.querySelector('img')).toBe(old);
    expect(f.views.read(f.view).selection?.sourceId).toBe(f.catalog.entries[0]!.source.id);
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('opens a clicked overview preview as a fitted image route', async () => {
    const f = fixture();
    const entry = f.catalog.entries[0]!;
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: 0,
      promptId: 'prompt:stream' as const,
      sourceId: entry.source.id,
      runOrder: 1,
      sequence: 1,
      token: 'overview-preview',
      dimensions: null,
    };
    f.replace({
      ...f.catalog,
      result: { ...f.catalog.result, runStatus: 'running', preview },
      entries: [{ ...entry, items: [], preview }],
    });
    f.views.update(f.view, (state) => ({ ...state, mode: 'overview' }));
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    const button = [...f.root.querySelectorAll('button')].find((element) =>
      element.getAttribute('aria-label')?.endsWith('Source preview'),
    );
    expect(button).toBeDefined();
    button!.click();
    await nextTick();
    expect(f.views.read(f.view)).toMatchObject({ mode: 'image', sourcePreview: true });
    expect(surface.wants(preview.sessionId, preview.generation, preview.sourceId)).toBe(true);
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('fits the first decoded image of a fresh Run while retaining inspection through same-Run replacements', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    const sources = f.catalog.entries.map((entry) => entry.source);
    f.replace(canvasCatalogFixture(2, 0, sources));
    surface.refresh();
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    await f.loader.ready(1);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('fit');
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    f.replace(canvasCatalogFixture(2, 0, sources, 'canvas', 3));
    surface.refresh();
    await f.loader.ready(2);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('starts a newly displayed preview fitted without inheriting or overwriting final inspection', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    const entry = f.catalog.entries[0]!;
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: f.catalog.generation,
      promptId: f.catalog.result.outputs[0]!.provenance.promptId!,
      sourceId: entry.source.id,
      runOrder: 1,
      sequence: 1,
      token: 'new-preview',
      dimensions: { width: 512, height: 288 },
    };
    f.replace({
      ...f.catalog,
      result: { ...f.catalog.result, preview },
      entries: [{ ...entry, preview }, ...f.catalog.entries.slice(1)],
    });
    surface.refresh();
    surface.receive(preview, new Blob(['preview']));
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    await f.loader.ready(1, preview.dimensions);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('fit');
    expect(surface.port.read().zoom.physicalScale).toBe(800 / 512);
    await f.command(surface, { type: 'zoom', factor: 2 });
    f.frames.advance(45);
    f.replace(f.catalog);
    surface.refresh();
    await f.command(surface, {
      type: 'select',
      selection: { sourceId: entry.source.id, batchIndex: 0 },
    });
    await f.loader.ready(2);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    surface.dispose();
    expect(f.errors).toEqual([]);
  });

  test('mounts full-fidelity admitted handles and controls with literal source labels and actual zoom status', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    expect(surface.port.read().resource.state).toBe('loading');
    const image = await f.loader.ready(0);
    f.clock.advance(100);
    f.frames.advance(16);
    await nextTick();
    expect(f.root.querySelector('img')).toBe(image);
    expect(f.root.textContent).toContain('<b>Output</b>');
    expect(f.root.querySelector('b')).toBeNull();
    expect(surface.port.read().resource.state).toBe('ready');
    expect(surface.port.read().zoom.dimensions).toEqual({ width: 3840, height: 2160 });
    expect((await f.command(surface, { type: 'native' })).status).toBe('applied');
    f.frames.advance(45);
    f.frames.advance(16);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    expect(image.style.transform).toContain('scale(1)');
    surface.dispose();
    expect(f.loader.requests[0]?.released).toBe(true);
    expect(f.loader.disposed).toBe(true);
    expect(f.root.children).toHaveLength(0);
    expect(f.views.accepts(f.address)).toBe(false);
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toEqual([]);
  });

  test('suspends interaction during source replacement without modifying the outgoing inspection', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    f.clock.advance(100);
    await f.command(surface, { type: 'link', enabled: false });
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    await f.command(surface, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    expect(surface.diagnostics().displayed).toBe(0);
    await f.command(surface, { type: 'zoom', factor: 2 });
    f.frames.advance(45);
    await f.loader.ready(1);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('fit');
    await f.command(surface, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[0]!.source.id, batchIndex: 0 },
    });
    await f.loader.ready(2);
    f.clock.advance(100);
    expect(surface.port.read().zoom.physicalScale).toBe(1);
    surface.dispose();
  });

  test('suspends zero-sized surfaces, refuses hidden preview interest, and restores light view state', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    f.bounds.mockReturnValue({
      width: 0,
      height: 0,
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
      toJSON: () => ({}),
    });
    window.dispatchEvent(new Event('resize'));
    f.frames.advance(16);
    expect(surface.diagnostics().displayed).toBe(0);
    expect(
      surface.wants(f.catalog.result.context.sessionId, 0, f.catalog.entries[0]!.source.id),
    ).toBe(false);
    f.bounds.mockReturnValue({
      width: 800,
      height: 600,
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 800,
      bottom: 600,
      toJSON: () => ({}),
    });
    window.dispatchEvent(new Event('resize'));
    f.frames.advance(16);
    await f.loader.ready(1);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    surface.dispose();
    expect(f.frames.pending).toBe(0);
  });

  test('contains late completion and stale commands after attachment retirement', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    const stale = surface.port.createRequest({ type: 'native' }, surface.port.read().revision);
    surface.dispose();
    await f.loader.ready(0);
    expect(f.loader.requests[0]?.released).toBe(true);
    expect(f.root.querySelector('img')).toBeNull();
    expect((await surface.port.dispatch(stale)).status).toBe('cancelled');
    expect(surface.diagnostics().pendingDecodes).toBe(0);
    expect(f.frames.pending).toBe(0);
  });

  test('cleans mounted controls, observers, and the lease when initial composition fails', () => {
    const f = fixture(true);
    expect(() => new BrowserCanvasAttachment(f.root, f.options)).toThrow(
      'File URL is unavailable.',
    );
    expect(f.loader.disposed).toBe(true);
    expect(f.root.children).toHaveLength(0);
    expect(f.views.accepts(f.address)).toBe(false);
    expect(f.frames.pending).toBe(0);
  });

  test('refreshes compared stage revisions and swaps admitted handles without reloads during wipe movement', async () => {
    const f = fixture();
    const read = jest.fn(f.options.catalog);
    const surface = new BrowserCanvasAttachment(f.root, { ...f.options, catalog: read });
    await f.loader.ready(0);
    f.clock.advance(100);
    await f.command(surface, { type: 'comparison', enabled: true });
    await f.loader.ready(1, { width: 2160, height: 3840 });
    f.clock.advance(100);
    f.frames.advance(16);
    expect(f.root.querySelectorAll('img')).toHaveLength(2);
    const scale = surface.port.read().zoom.secondaryPhysicalScale;
    expect(scale?.x).toBeCloseTo((800 / 3840) * (3840 / 2160));
    expect(scale?.y).toBeCloseTo((800 / 3840) * (2160 / 3840));
    const selected = surface.port.read().selected;
    const secondary = surface.port.read().secondary;
    f.replace(
      canvasCatalogFixture(
        1,
        0,
        f.catalog.entries.map((entry) => entry.source),
        'canvas',
        2,
      ),
    );
    surface.refresh();
    expect(surface.port.read().selected?.id).not.toBe(selected?.id);
    expect(surface.port.read().secondary?.id).not.toBe(secondary?.id);
    const refreshedSecondary = surface.port.read().secondary;
    const a = await f.loader.ready(2);
    const b = await f.loader.ready(3, { width: 2160, height: 3840 });
    f.clock.advance(100);
    const reads = read.mock.calls.length;
    for (let index = 0; index < 100; index++)
      await f.command(surface, { type: 'wipe', orientation: 'horizontal', split: index / 100 });
    expect(read.mock.calls).toHaveLength(reads);
    expect(f.loader.requests).toHaveLength(4);
    await f.command(surface, { type: 'swap' });
    f.clock.advance(100);
    f.frames.advance(16);
    expect(f.root.querySelector('.sc-output-image-layer img')).toBe(b);
    expect(f.root.querySelectorAll('img')).toContain(a);
    expect(f.loader.requests).toHaveLength(4);
    expect(surface.port.read().selected?.id).toBe(refreshedSecondary?.id);
    surface.dispose();
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toEqual([]);
  });

  test('renders only the selected source batch grid, preserves focus, and returns through selection authority', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    await f.command(surface, { type: 'overview', enabled: true });
    await nextTick();
    expect(f.root.classList.contains('sc-output-overview-mode')).toBe(true);
    expect(f.loader.requests[0]?.released).toBe(true);
    const cells = f.root.querySelectorAll<HTMLButtonElement>('.sc-output-overview-cell');
    expect(cells).toHaveLength(3);
    expect(cells[0]?.getAttribute('aria-label')).toContain('<b>Output</b>');
    expect(cells[0]?.textContent).not.toContain('<b>Output</b>');
    expect(f.root.querySelector('b')).toBeNull();
    expect(f.loader.requests.slice(1).every((request) => request.input.role === 'thumbnail')).toBe(
      true,
    );
    expect(surface.diagnostics().pendingDecodes).toBeLessThanOrEqual(2);
    await f.loader.ready(1);
    expect(f.root.querySelector('.sc-output-overview img')).not.toBeNull();
    expect(cells[0]?.textContent).toBe('');
    cells[1]?.focus();
    surface.refresh();
    expect(document.activeElement).toBe(cells[1]);
    expect(f.root.querySelectorAll('.sc-output-overview-cell')).toHaveLength(3);
    cells[1]?.click();
    expect(f.views.read(f.view).selection?.batchIndex).toBe(1);
    expect(f.views.read(f.view).mode).toBe('image');
    expect(surface.port.read().resource.state).toBe('unavailable');
    expect(surface.diagnostics().displayed).toBe(0);
    expect(surface.diagnostics().pendingDecodes).toBe(0);
    surface.dispose();
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toEqual([]);
  });

  test('retries a failed B independently and retains B when its A source is deleted', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    const a = await f.loader.ready(0);
    f.clock.advance(100);
    await f.command(surface, { type: 'comparison', enabled: true });
    await f.loader.fail(1);
    f.clock.advance(100);
    expect(surface.port.read().resource.state).toBe('ready');
    expect(surface.port.read().secondaryResource?.state).toBe('error');
    expect(f.root.querySelector('img')).toBe(a);
    await f.command(surface, { type: 'retry' });
    expect(f.loader.requests).toHaveLength(3);
    const b = await f.loader.ready(2, { width: 2160, height: 3840 });
    f.replace({ ...f.catalog, entries: f.catalog.entries.slice(1) });
    surface.refresh();
    f.frames.advance(16);
    expect(surface.port.read().resource.state).toBe('unavailable');
    expect(surface.port.read().resource.message).toContain('Showing B.');
    expect(surface.port.read().zoom.dimensions).toEqual({ width: 2160, height: 3840 });
    expect(f.root.querySelectorAll('img')).toHaveLength(1);
    expect(f.root.querySelector('img')).toBe(b);
    expect(f.loader.requests[0]?.released).toBe(true);
    surface.dispose();
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toHaveLength(1);
  });

  test('bounds mounted gallery cells and resource interests while scrolling a large batch', async () => {
    const f = fixture();
    const entry = f.catalog.entries[0]!;
    const item = entry.items[0]!;
    f.replace({
      ...f.catalog,
      entries: [
        {
          ...entry,
          summary: { ...entry.summary, batchCount: 100000 },
          items: Array.from({ length: 32 }, (_, batchIndex) => ({
            ...item,
            id: `result:gallery:${batchIndex}`,
            batchIndex,
            dimensions: { width: 128, height: 128 },
          })),
        },
      ],
    });
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.command(surface, { type: 'overview', enabled: true });
    const gallery = f.root.querySelector<HTMLElement>('.sc-output-overview')!;
    for (let step = 0; step < 100; step++) {
      gallery.scrollTop = step * 200 + 0.5;
      gallery.dispatchEvent(new Event('scroll'));
      gallery.dispatchEvent(new Event('scroll'));
      expect(f.frames.pending).toBeLessThanOrEqual(2);
      f.frames.advance(16);
      expect(gallery.querySelectorAll('button').length).toBeLessThanOrEqual(8);
      expect(surface.diagnostics().pendingDecodes).toBeLessThanOrEqual(2);
      expect(surface.diagnostics().queuedInputs).toBeLessThanOrEqual(8);
      expect(surface.diagnostics().estimatedBytes).toBeLessThanOrEqual(128 * 1024 * 1024);
    }
    expect(surface.wants(f.catalog.result.context.sessionId, 0, entry.source.id)).toBe(false);
    surface.dispose();
    for (let index = 0; index < f.loader.requests.length; index++) await f.loader.ready(index);
    expect(f.loader.requests.every((request) => request.released)).toBe(true);
    expect(surface.diagnostics().pendingDecodes).toBe(0);
    expect(surface.diagnostics().estimatedBytes).toBe(0);
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toEqual([]);
  });
});
