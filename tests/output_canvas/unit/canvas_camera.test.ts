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
/** Prove anchored navigation, physical density, snapping and bounded inspection without a renderer. */

import {
  anchoredCanvasCamera,
  canvasCameraGeometry,
  canvasFitScale,
  canvasWheelFactor,
  centeredCanvasCamera,
  doubleClickCanvasCamera,
  panCanvasCamera,
  settleCanvasCamera,
  zoomCanvasCamera,
  type CanvasCameraGeometry,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';
import {
  captureCanvasInspection,
  projectCanvasInspection,
  resizeCanvasCamera,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasInspection.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasCameraTransition } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasCameraTransition.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Construct validated geometry with convenient dimensions for exact snap assertions. */
function geometry(
  width = 4000,
  height = 2000,
  density = 1,
  viewportWidth = 1000,
  viewportHeight = 500,
): CanvasCameraGeometry {
  const result = canvasCameraGeometry(
    { width, height },
    { width: viewportWidth, height: viewportHeight },
    density,
  );
  if (!result) throw new Error('Camera fixture geometry is invalid.');
  return result;
}

describe('browser canvas camera', () => {
  test.each([1, 1.25, 1.5, 2])('uses physical native pixels at density %s', (density) => {
    const g = geometry(4000, 2000, density);
    expect(centeredCanvasCamera(g, 'native').scale * density).toBe(1);
    expect(canvasFitScale(g)).toBe(0.25);
    expect(centeredCanvasCamera(g)).toMatchObject({ scale: 0.25, x: 0, y: 0, mode: 'fit' });
  });

  test('rejects missing, hidden and nonfinite geometry before saving a camera', () => {
    expect(
      canvasCameraGeometry({ width: 0.5, height: 100 }, { width: 100, height: 100 }, 1),
    ).toBeNull();
    expect(
      canvasCameraGeometry(
        { width: 4000, height: 2000 },
        { width: 100, height: 100 },
        Number.MIN_VALUE,
      ),
    ).toBeNull();
    expect(
      canvasCameraGeometry(
        { width: 4000, height: 2000 },
        { width: Number.MIN_VALUE, height: 100 },
        1,
      ),
    ).toBeNull();
    expect(canvasCameraGeometry(null, { width: 100, height: 100 }, 1)).toBeNull();
    for (const invalid of [0, -1, NaN, Infinity]) {
      expect(
        canvasCameraGeometry({ width: invalid, height: 100 }, { width: 100, height: 100 }, 1),
      ).toBeNull();
      expect(
        canvasCameraGeometry({ width: 100, height: 100 }, { width: invalid, height: 100 }, 1),
      ).toBeNull();
      expect(
        canvasCameraGeometry({ width: 100, height: 100 }, { width: 100, height: 100 }, invalid),
      ).toBeNull();
    }
  });

  test('preserves an exact image point under an arbitrary zoom anchor before constraints', () => {
    const g = geometry();
    const start = centeredCanvasCamera(g);
    const anchor = { x: 321, y: 123 };
    const next = anchoredCanvasCamera(start, 0.75, anchor, g);
    expect((anchor.x - next.x) / next.scale).toBeCloseTo((anchor.x - start.x) / start.scale);
    expect((anchor.y - next.y) / next.scale).toBeCloseTo((anchor.y - start.y) / start.scale);
    expect(() => anchoredCanvasCamera(start, NaN, anchor, g)).toThrow('invalid');
  });

  test('snaps in both directions, prioritizes native when both targets are crossed, and departs next input', () => {
    const g = geometry();
    const anchor = { x: 500, y: 250 };
    const fit = centeredCanvasCamera(g);
    const crossed = zoomCanvasCamera(fit, 8, anchor, g);
    expect(crossed.scale).toBe(1);
    expect(zoomCanvasCamera(crossed, 1.25, anchor, g).scale).toBe(1.25);
    const down = zoomCanvasCamera(zoomCanvasCamera(crossed, 2, anchor, g), 0.01, anchor, g);
    expect(down.scale).toBe(1);
    const downFit = zoomCanvasCamera(down, 0.1, anchor, g);
    expect(downFit.scale).toBe(0.25);
    expect(zoomCanvasCamera(downFit, 1.25, anchor, g).scale).toBe(0.3125);
    expect(settleCanvasCamera(zoomCanvasCamera(crossed, 1000, anchor, g), g).scale).toBe(10);
  });

  test('accumulates fractional wheel movement and normalizes line/page units', () => {
    expect(canvasWheelFactor(-100, 0, 500)).toBeCloseTo(1.25);
    expect(Math.pow(canvasWheelFactor(-1, 0, 500), 100)).toBeCloseTo(1.25);
    expect(canvasWheelFactor(3, 1, 500)).toBe(canvasWheelFactor(48, 0, 500));
    expect(canvasWheelFactor(1, 2, 500)).toBe(canvasWheelFactor(500, 0, 500));
    expect(canvasWheelFactor(NaN, 0, 500)).toBe(1);
  });

  test('bounds overflowing axes and centers fitting axes while allowing transient anchored slack', () => {
    const g = geometry();
    const native = centeredCanvasCamera(g, 'native');
    const far = panCanvasCamera(native, { x: 1e6, y: -1e6 }, g);
    expect(far.x).toBe(200);
    expect(far.y).toBe(-1600);
    const fit = centeredCanvasCamera(g);
    const transient = anchoredCanvasCamera(native, fit.scale, { x: 0, y: 0 }, g);
    expect(transient.x).not.toBe(fit.x);
    expect(settleCanvasCamera(transient, g)).toEqual(fit);
    expect(panCanvasCamera(fit, { x: 1000, y: 1000 }, g)).toEqual(fit);
  });

  test('toggles Fit/native and keeps tiny images centered without upscaling or oscillation', () => {
    const g = geometry();
    const fit = centeredCanvasCamera(g);
    const native = doubleClickCanvasCamera(fit, { x: 350, y: 200 }, g);
    expect(native.mode).toBe('native');
    expect(doubleClickCanvasCamera(native, { x: 350, y: 200 }, g)).toEqual(fit);
    const tiny = geometry(100, 50, 2);
    const smallFit = centeredCanvasCamera(tiny);
    expect(smallFit.scale).toBe(0.5);
    expect(doubleClickCanvasCamera(smallFit, { x: 0, y: 0 }, tiny)).toEqual(smallFit);
  });

  test('projects image-relative magnification conservatively and never borrows a native label', () => {
    const source = geometry();
    const native = panCanvasCamera(
      centeredCanvasCamera(source, 'native'),
      { x: 200, y: 100 },
      source,
    );
    const inspection = captureCanvasInspection(native, source);
    const bigger = geometry(8000, 4000, 2);
    const projected = projectCanvasInspection(inspection, bigger);
    expect(projected.scale * bigger.density).toBe(0.5);
    expect(projected.mode).toBe('custom');
    expect(captureCanvasInspection(projected, bigger).region.centerX).toBeCloseTo(
      inspection.region.centerX,
    );
    const portrait = geometry(2000, 4000);
    const receiving = projectCanvasInspection(inspection, portrait);
    expect(receiving.scale).toBe(0.5);
    expect(receiving.mode).toBe('custom');
  });

  test('resizes custom/native cameras at stable physical scale and recalculates Fit', () => {
    const before = geometry();
    const after = geometry(4000, 2000, 2, 1200, 600);
    const custom = settleCanvasCamera(
      anchoredCanvasCamera(centeredCanvasCamera(before), 0.75, { x: 250, y: 125 }, before),
      before,
    );
    const resized = resizeCanvasCamera(custom, before, after);
    expect(resized.scale * after.density).toBeCloseTo(custom.scale);
    expect(captureCanvasInspection(resized, after).region.centerX).toBeCloseTo(
      captureCanvasInspection(custom, before).region.centerX,
    );
    expect(resizeCanvasCamera(centeredCanvasCamera(before, 'native'), before, after).scale).toBe(
      0.5,
    );
    expect(resizeCanvasCamera(centeredCanvasCamera(before), before, after)).toEqual(
      centeredCanvasCamera(after),
    );
  });

  test('scopes displayed-Run resets to one view and ignores repeated or stale Run identities', () => {
    const store = new CanvasInspectionStore();
    const a = 'view:a' as const;
    const b = 'view:b' as const;
    const slot = { sourceId: 'source:1' as const, batchIndex: 0 };
    const g = geometry();
    const native = centeredCanvasCamera(g, 'native');
    for (const view of [a, b]) {
      store.synchronize(view, 0, [slot]);
      expect(store.presentRun(view, 1)).toBe(true);
      store.capture(view, slot, native, g);
    }
    expect(store.presentRun(a, 2)).toBe(true);
    expect(store.camera(a, slot, g).mode).toBe('fit');
    expect(store.camera(b, slot, g)).toEqual(native);
    store.capture(a, slot, native, g);
    expect(store.presentRun(a, 2)).toBe(false);
    expect(store.presentRun(a, 1)).toBe(false);
    expect(store.camera(a, slot, g)).toEqual(native);
    store.synchronize(a, 1, [slot]);
    expect(store.presentRun(a, 1)).toBe(true);
    expect(store.camera(a, slot, g).mode).toBe('fit');
    store.dispose();
  });

  test('keeps linked views and independent slots separate, pruning replacement generations', () => {
    const store = new CanvasInspectionStore();
    const a = 'view:a' as const;
    const b = 'view:b' as const;
    const first = { sourceId: 'source:1' as const, batchIndex: 0 };
    const second = { sourceId: 'source:2' as const, batchIndex: 0 };
    const g = geometry();
    const native = centeredCanvasCamera(g, 'native');
    store.synchronize(a, 0, [first, second]);
    store.synchronize(b, 0, [first, second]);
    store.capture(a, first, native, g);
    expect(store.camera(a, second, g)).toEqual(native);
    expect(store.camera(b, first, g).mode).toBe('fit');
    store.setLinked(a, false, { selection: first, camera: native, geometry: g });
    expect(store.camera(a, second, g).mode).toBe('fit');
    store.capture(a, first, native, g);
    expect(store.camera(a, first, g)).toEqual(native);
    expect(store.camera(a, second, g).mode).toBe('fit');
    store.synchronize(a, 0, [second]);
    expect(store.camera(a, first, g).mode).toBe('fit');
    store.setLinked(a, true, null);
    expect(store.camera(a, second, g)).toEqual(native);
    store.synchronize(a, 1, [first, second]);
    expect(store.camera(a, first, g).mode).toBe('fit');
    store.remove(a);
    expect(() => store.camera(a, first, g)).toThrow('unavailable');
    store.dispose();
  });

  test('animates only while active, rebases interruption, and removes smoothing for bursts/reduced motion', () => {
    const clock = new CanvasFrameFixture();
    const g = geometry();
    const rendered: number[] = [];
    const transition = new CanvasCameraTransition(clock, (camera) => rendered.push(camera.scale));
    const fit = centeredCanvasCamera(g);
    transition.set(fit);
    transition.move(centeredCanvasCamera(g, 'native'), g, { x: 500, y: 250 }, false, true);
    expect(clock.pending).toBe(1);
    clock.advance(16);
    const displayed = transition.read();
    if (!displayed) throw new Error('Displayed camera is missing.');
    expect(displayed.scale).toBeGreaterThan(fit.scale);
    expect(displayed.scale).toBeLessThan(1);
    const target = settleCanvasCamera(zoomCanvasCamera(displayed, 1.25, { x: 500, y: 250 }, g), g);
    transition.move(target, g, { x: 500, y: 250 }, false, true);
    expect(transition.read()).toEqual(target);
    expect(clock.pending).toBe(0);
    clock.advance(100);
    transition.move(fit, g, null, true);
    expect(transition.read()).toEqual(fit);
    expect(clock.pending).toBe(0);
    transition.move(centeredCanvasCamera(g, 'native'), g, null, false);
    transition.dispose();
    clock.advance(100);
    expect(clock.pending).toBe(0);
    expect(transition.read()).toBeNull();
    expect(rendered.length).toBeGreaterThan(1);
  });

  test('settles a closely spaced wheel transition in twenty milliseconds', () => {
    const clock = new CanvasFrameFixture();
    const g = geometry();
    const transition = new CanvasCameraTransition(clock, () => undefined);
    transition.set(centeredCanvasCamera(g));
    transition.move(centeredCanvasCamera(g, 'native'), g, null, false, true);
    clock.advance(50);
    const target = centeredCanvasCamera(g);
    transition.move(target, g, null, false, true);
    clock.advance(10);
    expect(transition.read()?.scale).toBeGreaterThan(target.scale);
    clock.advance(10);
    expect(transition.read()).toEqual(target);
    expect(clock.pending).toBe(0);
  });

  test('rejects a queued callback already delivered when another callback disposes its attachment', () => {
    const clock = new CanvasFrameFixture();
    const g = geometry();
    const rendered: number[] = [];
    const transition = new CanvasCameraTransition(clock, (camera) => rendered.push(camera.scale));
    transition.set(centeredCanvasCamera(g));
    clock.request(() => transition.dispose());
    transition.move(centeredCanvasCamera(g, 'native'), g, null, false);
    clock.advance(16);
    expect(rendered).toHaveLength(1);
    expect(clock.pending).toBe(0);
  });

  test('returns to Fit with monotonically decreasing displacement from image center', () => {
    const clock = new CanvasFrameFixture();
    const g = geometry();
    const displacement: number[] = [];
    const transition = new CanvasCameraTransition(clock, (camera) =>
      displacement.push(camera.x - (g.viewport.width - g.image.width * camera.scale) / 2),
    );
    transition.set(panCanvasCamera(centeredCanvasCamera(g, 'native'), { x: 300, y: 0 }, g));
    transition.move(centeredCanvasCamera(g), g, null, false);
    for (let index = 0; index < 5; index++) clock.advance(10);
    expect(displacement.at(-1)).toBe(0);
    expect(
      displacement.every(
        (value, index) =>
          value >= 0 && (index === 0 || value <= (displacement[index - 1] ?? value)),
      ),
    ).toBe(true);
    expect(clock.pending).toBe(0);
  });
});
