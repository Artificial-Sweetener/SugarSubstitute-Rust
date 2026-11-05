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
    mediaUrl: (value) => `/view?id=${value.id}`,
  });
}

test('the native snapshot places the painted image from the surface camera', () => {
  const backdrop = snapshot().backdrop;
  expect(backdrop?.rect).toEqual({ left: 200, top: 50, width: 800, height: 400 });
  expect(backdrop?.url).toContain('/view?id=');
  expect(snapshot('overview').backdrop).toBeNull();
});

test('the navigation bar blurs that image beneath itself, aligned with the surface', () => {
  const native = snapshot();
  const root = document.createElement('section');
  document.body.append(root);
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
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
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture({ ...canvasSnapshot('native'), backdrop: native.backdrop ?? null }),
    jest.fn(),
  );
  const image = root.querySelector<HTMLImageElement>('.sc-output-tabstrip .sc-output-backdrop img');
  expect(image?.getAttribute('src')).toBe(native.backdrop?.url);
  expect([image?.style.left, image?.style.top, image?.style.width]).toEqual([
    '92px',
    '-358px',
    '800px',
  ]);
  mounted.dispose();
});
