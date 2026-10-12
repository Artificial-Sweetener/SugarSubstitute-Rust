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
/** Prove admitted elements render without copies, unbounded frames, or cross-root teardown. */

import { BrowserCanvasImages } from '../../../frontend/comfyui/ui/output_canvas/presentation/browser/BrowserCanvasImages.js';
import {
  canvasCameraGeometry,
  centeredCanvasCamera,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import type { CanvasImageFrame } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';

/** Supply an already decoded platform handle without a network or image-load owner. */
function frame(key: string, width = 3840, height = 2160): CanvasImageFrame<HTMLImageElement> {
  const handle = document.createElement('img');
  handle.src = `/view?filename=${key}`;
  handle.draggable = false;
  return {
    key,
    selection: { sourceId: outputSourceId(key), batchIndex: 0 },
    preview: false,
    dimensions: { width, height },
    handle,
  };
}

describe('browser image presentation', () => {
  test('coalesces transforms into one frame and renders the admitted handle without a second source', () => {
    const root = document.createElement('div');
    const clock = new CanvasFrameFixture();
    const surface = new BrowserCanvasImages(root, clock);
    const image = frame('a');
    const geometry = canvasCameraGeometry(image.dimensions, { width: 800, height: 600 }, 1)!;
    surface.images(image, null);
    for (let step = 0; step < 100; step++)
      surface.camera({
        selection: image.selection,
        geometry,
        camera: { scale: 2, x: -step, y: -20, mode: 'custom' },
      });
    expect(clock.pending).toBe(1);
    clock.advance(16);
    expect(root.querySelectorAll('img')).toHaveLength(1);
    expect(root.querySelector('img')).toBe(image.handle);
    expect(image.handle.getAttribute('src')).toBe('/view?filename=a');
    expect(image.handle.style.transform).toBe('translate(-99px, -20px) scale(2)');
    expect(image.handle.style.imageRendering).toBe('pixelated');
    expect(clock.pending).toBe(0);
    expect(surface.contains({ x: 10, y: 10 })).toBe(true);
    expect(surface.contains({ x: 900, y: 10 })).toBe(false);
    surface.dispose();
    expect(root.children).toHaveLength(0);
    expect(image.handle.getAttribute('src')).toBe('/view?filename=a');
  });

  test('registers B to A with truthful filtering and changes wipe without reallocating image nodes', () => {
    const root = document.createElement('div');
    const clock = new CanvasFrameFixture();
    const surface = new BrowserCanvasImages(root, clock);
    const a = frame('a');
    const b = frame('b', 1920, 1080);
    const geometry = canvasCameraGeometry(a.dimensions, { width: 800, height: 600 }, 2)!;
    surface.images(a, b);
    surface.camera({
      selection: a.selection,
      geometry,
      camera: { scale: 0.5, x: 0, y: 0, mode: 'native' },
    });
    surface.wipe({ split: 0.3, orientation: 'vertical' });
    clock.advance(16);
    expect(b.handle.style.width).toBe('3840px');
    expect(a.handle.style.imageRendering).toBe('auto');
    expect(b.handle.style.imageRendering).toBe('pixelated');
    const layer = b.handle.parentElement!;
    expect(layer.style.clipPath).toBe('inset(0 70% 0 0)');
    surface.wipe({ split: 0.7, orientation: 'horizontal' });
    clock.advance(16);
    expect(b.handle.parentElement).toBe(layer);
    expect(layer.style.clipPath).toContain('inset(0 0 30');
    surface.images(b, a);
    clock.advance(16);
    expect(root.querySelectorAll('img')).toHaveLength(2);
    surface.dispose();
  });

  test('cancels hidden/disposed work independently and schedules no idle frames', () => {
    const clock = new CanvasFrameFixture();
    const first = new BrowserCanvasImages(document.createElement('div'), clock);
    const second = new BrowserCanvasImages(document.createElement('div'), clock);
    const a = frame('a');
    const b = frame('b');
    const geometry = canvasCameraGeometry(a.dimensions, { width: 800, height: 600 }, 1)!;
    first.images(a, null);
    second.images(b, null);
    first.camera({ selection: a.selection, geometry, camera: centeredCanvasCamera(geometry) });
    second.camera({ selection: b.selection, geometry, camera: centeredCanvasCamera(geometry) });
    first.camera(null);
    expect(clock.pending).toBe(1);
    expect(first.contains({ x: 100, y: 100 })).toBe(false);
    first.dispose();
    clock.advance(16);
    expect(b.handle.style.transform).not.toBe('');
    expect(clock.pending).toBe(0);
    for (let idle = 0; idle < 100; idle++) clock.advance(16);
    expect(clock.pending).toBe(0);
    second.dispose();
    second.images(b, null);
    expect(clock.pending).toBe(0);
  });
});
