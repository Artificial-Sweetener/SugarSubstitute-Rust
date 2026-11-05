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
/** Prove glass chrome over a native surface mirrors the painted image where it lies on screen. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { nativeCanvasSnapshot } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasSnapshot.js';
import { initialCanvasViewState } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewState.js';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Snapshot one natively painted image at a camera centred on its point (0.25, 0.5) at 2x. */
function snapshot(mode: 'image' | 'overview' = 'image') {
  const catalog = canvasCatalogFixture();
  const entry = catalog.entries[0]!;
  const view = {
    ...initialCanvasViewState({ sessionId: catalog.sessionId, viewId: 'canvas-view:a' as never }),
    selection: { sourceId: entry.source.id, batchIndex: 0 },
    mode,
  };
  const item = entry.items[0]!;
  const plan = canvasDisplayPlan(view, catalog, { primary: item, secondary: null }, null);
  return nativeCanvasSnapshot({
    address: canvasSnapshot('native').address,
    plan,
    catalog,
    sources: catalog.entries.map((value) => value.summary),
    connected: true,
    mediaActions: null,
    presentation: { state: 'ready', message: '' },
    viewport: {
      mode: 'custom',
      physicalScale: 0.5,
      dimensions: { width: 1600, height: 800 },
      center: { x: 0.25, y: 0.5 },
    },
    preview: false,
    previewAvailable: false,
    comparison: false,
    overview: true,
    geometry: {
      coordinateSpace: 'client',
      units: 'css-pixels',
      origin: { x: 100, y: 50 },
      viewport: { width: 600, height: 400 },
      clip: { x: 0, y: 0, width: 600, height: 400 },
      physicalPixelsPerCssPixel: 1,
      visible: true,
      focused: false,
    },
    backdropUrl: (value) => `/preview?id=${value.id}`,
  });
}

test('the native snapshot places the painted image from the surface camera', () => {
  const backdrop = snapshot().backdrop;
  expect(backdrop?.rect).toEqual({ left: 200, top: 50, width: 800, height: 400 });
  expect(backdrop?.url).toContain('/preview?id=');
  expect(snapshot('overview').backdrop).toBeNull();
});

test('the navigation bar paints the preview as its own blurred background, aligned with the surface', async () => {
  const native = snapshot();
  const root = document.createElement('section');
  document.body.append(root);
  const bounds = jest
    .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
    .mockImplementation(function (this: HTMLElement) {
      const [left, top, width, height] = this.classList.contains('sc-output-tabstrip')
        ? [108, 408, 300, 38]
        : [0, 0, 0, 0];
      return {
        x: left,
        y: top,
        left,
        top,
        width,
        height,
        right: left + width,
        bottom: top + height,
        toJSON: () => ({}),
      };
    });
  const port = new CanvasPortFixture({
    ...canvasSnapshot('native'),
    backdrop: native.backdrop ?? null,
  });
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  const layer = root.querySelector<HTMLElement>(
    '.sc-output-tabstrip .sc-output-backdrop .sc-output-backdrop-image',
  );
  // No image-sized element: the bar-sized layer carries the scaled preview as its background.
  expect(root.querySelector('.sc-output-backdrop img')).toBeNull();
  expect(layer?.style.backgroundImage).toContain(native.backdrop?.url);
  expect(layer?.style.backgroundSize).toBe('800px 400px');
  // Image left 200 - bar 108 + 24 blur margin; image top 50 - bar 408 + 24.
  expect(layer?.style.backgroundPosition).toBe('116px -334px');
  // A camera update repositions the background without reading layout again.
  const reads = bounds.mock.calls.length;
  const container = root.querySelector('.sc-output-tabstrip')!.parentElement!;
  const widths = jest.spyOn(container, 'clientWidth', 'get');
  const backdrop = native.backdrop!;
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    backdrop: { ...backdrop, rect: { ...backdrop.rect, left: 150, width: 1600, height: 800 } },
  });
  await nextTick();
  expect(layer?.style.backgroundSize).toBe('1600px 800px');
  expect(layer?.style.backgroundPosition).toBe('66px -334px');
  expect(bounds.mock.calls.length).toBe(reads);
  expect(widths).not.toHaveBeenCalled();
  mounted.dispose();
});

test('the blur covers only the bar and its margin, never the whole image', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('native')),
    jest.fn(),
  );
  const rules = [...document.styleSheets].flatMap((sheet) => [...sheet.cssRules]);
  const rule = (selector: string) =>
    rules.find(
      (value): value is CSSStyleRule =>
        value instanceof CSSStyleRule && value.selectorText === selector,
    )?.style;
  const layer = rule('.sc-output-backdrop-image');
  expect(layer?.getPropertyValue('filter')).toBe('blur(var(--p-toast-blur,10px))');
  expect(layer?.getPropertyValue('inset')).toBe('-24px');
  expect(rule('.sc-output-backdrop')?.getPropertyValue('overflow')).toBe('hidden');
  expect(rule('.sc-output-backdrop img')).toBeUndefined();
  mounted.dispose();
});
