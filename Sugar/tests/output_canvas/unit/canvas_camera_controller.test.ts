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
/** Prove attachment camera orchestration restores durable inspection without idle or stale frames. */

import { CanvasCameraController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasCameraController.js';
import {
  CanvasInspectionStore,
  type CanvasInspectionTarget,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { captureCanvasInspection } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasInspection.js';
import { canvasCameraGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

/** Enroll a real view and inspect visible presentation without DOM or host globals. */
function fixture(reduced = false) {
  const catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'camera');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const slots = catalog.entries.map((entry) => ({ sourceId: entry.source.id, batchIndex: 0 }));
  const inspections = new CanvasInspectionStore();
  inspections.synchronize(view, catalog.generation, slots);
  const clock = new CanvasFrameFixture();
  const shown: (CanvasInspectionTarget | null)[] = [];
  const camera = new CanvasCameraController({
    view,
    inspections,
    clock,
    reducedMotion: () => reduced,
    present: (target) => {
      shown.push(target);
    },
  });
  const geometry = canvasCameraGeometry(
    { width: 3840, height: 2160 },
    { width: 800, height: 600 },
    1,
  )!;
  return { catalog, views, view, slots, inspections, clock, shown, camera, geometry };
}

describe('canvas camera attachment orchestration', () => {
  test('locks every displayed preview to Fit while preserving final inspection', () => {
    const f = fixture(true);
    const a = f.slots[0]!;
    const b = f.slots[1]!;
    f.camera.show(a, f.geometry, 1, null);
    f.camera.native();
    const preview = canvasCameraGeometry({ width: 512, height: 288 }, f.geometry.viewport, 1)!;
    f.camera.show(a, preview, 1, 'preview:A');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    expect(f.camera.read()?.camera.scale).toBe(800 / 512);
    const fitted = f.camera.read()!.camera;
    expect(f.camera.fit()).toBe(false);
    expect(f.camera.native()).toBe(false);
    expect(f.camera.zoom(1.25)).toBe(false);
    expect(f.camera.pan({ x: -10, y: -10 })).toBe(false);
    expect(f.camera.pinch(2, { x: 200, y: 100 }, { x: 20, y: 10 })).toBe(false);
    expect(f.camera.wheel(-10, 0, { x: 200, y: 100 })).toBe(false);
    expect(f.camera.doubleClick({ x: 200, y: 100 })).toBe(false);
    f.camera.show(a, preview, 1, 'preview:A');
    expect(f.camera.read()?.camera).toEqual(fitted);
    f.camera.show(b, preview, 1, 'preview:B');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    expect(f.camera.read()?.camera.scale).toBe(800 / 512);
    f.camera.show(a, preview, 1, 'preview:A');
    expect(f.camera.read()?.camera).toEqual(fitted);
    const resized = canvasCameraGeometry(preview.image, { width: 600, height: 400 }, 2)!;
    f.camera.show(a, resized, 1, 'preview:A');
    expect(f.camera.read()?.camera).toMatchObject({ mode: 'fit', scale: 600 / 512 });
    f.camera.show(b, f.geometry, 1, null);
    expect(f.camera.read()?.camera.mode).toBe('native');
    expect(f.camera.zoom(2)).toBe(true);
    f.camera.show(a, preview, 1, 'preview:A:replacement');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    expect(f.camera.zoom(1.25)).toBe(false);
    f.camera.suspend();
    f.camera.show(a, preview, 1, 'preview:A:replacement');
    expect(f.camera.read()?.camera).toEqual(fitted);
    f.camera.show(b, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBe(2);
    f.camera.show(a, preview, 2, 'preview:A:new-run');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.dispose();
    f.inspections.dispose();
  });

  test('retires final animation immediately when preview pixels replace the final image', () => {
    const f = fixture();
    const slot = f.slots[0]!;
    f.camera.show(slot, f.geometry, 1, null);
    f.camera.native();
    f.clock.advance(10);
    const final = f.camera.read()!.camera;
    expect(f.clock.pending).toBe(1);
    f.camera.show(slot, f.geometry, 1, 'preview:stream');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    expect(f.clock.pending).toBe(0);
    f.clock.advance(100);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.show(slot, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBeCloseTo(final.scale);
    expect(f.camera.zoom(2)).toBe(true);
    f.camera.dispose();
    f.inspections.dispose();
  });

  test('preserves final Run inspection through a newer preview and retires it only when the newer final appears', () => {
    const f = fixture(true);
    const slot = f.slots[0]!;
    f.camera.show(slot, f.geometry, 1, null);
    f.camera.native();
    f.camera.zoom(2);
    f.camera.pan({ x: -120, y: 70 });
    const inspected = f.camera.read()!.camera;
    f.camera.show(slot, f.geometry, 2, 'preview:run2');
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.show(slot, f.geometry, 1, null);
    expect(f.camera.read()?.camera).toEqual(inspected);
    f.camera.show(slot, f.geometry, 2, 'preview:run2');
    f.camera.show(slot, f.geometry, 2, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.show(f.slots[1]!, f.geometry, 2, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.dispose();
    f.inspections.dispose();
  });
  test('resets shared and independent inspection once per displayed Run without carrying a comparison camera', () => {
    const f = fixture(true);
    const a = f.slots[0]!;
    const b = f.slots[1]!;
    f.camera.show(a, f.geometry, 1, null);
    f.camera.native();
    f.camera.link(false);
    f.camera.show(b, f.geometry, 1, null);
    f.camera.native();
    f.camera.comparison(true);
    f.camera.show(a, f.geometry, 2, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.comparison(false);
    f.camera.show(b, f.geometry, 2, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.native();
    f.camera.pan({ x: -40, y: -20 });
    const adjusted = f.camera.read()!.camera;
    f.camera.show(b, f.geometry, 2, null);
    expect(f.camera.read()?.camera).toEqual(adjusted);
    f.camera.suspend();
    f.camera.show(b, f.geometry, 2, null);
    expect(f.camera.read()?.camera).toMatchObject({ mode: adjusted.mode, scale: adjusted.scale });
    expect(f.camera.read()?.camera.x).toBeCloseTo(adjusted.x);
    expect(f.camera.read()?.camera.y).toBeCloseTo(adjusted.y);
    f.camera.link(true);
    f.camera.show(b, f.geometry, 3, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.dispose();
    f.inspections.dispose();
  });
  test('retains the common comparison region when solo cameras are unlinked', () => {
    const f = fixture(true);
    f.camera.show(f.slots[0]!, f.geometry, 1, null);
    f.camera.link(false);
    f.camera.native();
    f.camera.pan({ x: -40, y: -20 });
    f.camera.comparison(true);
    const smaller = canvasCameraGeometry({ width: 1920, height: 1080 }, f.geometry.viewport, 1)!;
    f.camera.show(f.slots[1]!, smaller, 1, null);
    expect(f.camera.read()?.camera.scale).toBe(2);
    expect(f.camera.read()?.camera.mode).toBe('custom');
    f.camera.comparison(false);
    f.camera.show(f.slots[2]!, f.geometry, 1, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.show(f.slots[0]!, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBe(1);
    f.camera.dispose();
    expect(f.clock.pending).toBe(0);
  });
  test('preserves Auto during inspection and ignores repeated unchanged preview geometry', () => {
    const f = fixture();
    const slot = f.slots[0]!;
    expect(f.camera.fit()).toBe(false);
    f.camera.show(slot, f.geometry, 1, null);
    expect(f.views.read(f.view).follow).toBe('auto');
    expect(f.camera.native()).toBe(true);
    expect(f.clock.pending).toBe(1);
    f.clock.advance(45);
    expect(f.camera.read()?.camera.mode).toBe('native');
    expect(f.views.read(f.view).follow).toBe('auto');
    f.camera.pan({ x: -30, y: -20 });
    const count = f.shown.length;
    for (let frame = 0; frame < 100; frame++) f.camera.show(slot, f.geometry, 1, null);
    expect(f.shown).toHaveLength(count);
    expect(f.clock.pending).toBe(0);
    f.camera.dispose();
    f.inspections.dispose();
  });

  test('preserves native scale for display-density resize but scene inspection for image replacement', () => {
    const f = fixture(true);
    const slot = f.slots[0]!;
    f.camera.show(slot, f.geometry, 1, null);
    f.camera.native();
    const resized = canvasCameraGeometry(f.geometry.image, { width: 900, height: 650 }, 2)!;
    f.camera.show(slot, resized, 1, null);
    expect(f.camera.read()?.camera).toMatchObject({ scale: 0.5, mode: 'native' });
    const replaced = canvasCameraGeometry({ width: 7680, height: 4320 }, resized.viewport, 2)!;
    f.camera.show(slot, replaced, 1, null);
    expect(f.camera.read()?.camera).toMatchObject({ scale: 0.25, mode: 'custom' });
    f.camera.dispose();
    f.inspections.dispose();
  });

  test('restores independent slot cameras and deliberately seeds shared inspection when linking', () => {
    const f = fixture(true);
    const a = f.slots[0]!;
    const b = f.slots[1]!;
    f.camera.show(a, f.geometry, 1, null);
    f.camera.native();
    f.camera.link(false);
    f.camera.show(b, f.geometry, 1, null);
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.native();
    f.camera.zoom(2);
    expect(f.camera.read()?.camera.scale).toBe(2);
    f.camera.show(a, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBe(1);
    f.camera.link(true);
    f.camera.show(b, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBe(1);
    f.camera.link(false);
    expect(f.camera.read()?.camera.scale).toBe(2);
    f.camera.dispose();
    f.inspections.dispose();
  });

  test('retires suspended tween frames and restores the displayed origin on reactivation', () => {
    const f = fixture();
    const slot = f.slots[0]!;
    f.camera.show(slot, f.geometry, 1, null);
    f.camera.native();
    f.clock.advance(10);
    const displayed = f.camera.read()!;
    expect(displayed.camera.mode).toBe('custom');
    f.camera.suspend();
    expect(f.camera.read()).toBeNull();
    expect(f.clock.pending).toBe(0);
    expect(f.camera.pan({ x: 10, y: 10 })).toBe(false);
    f.camera.show(slot, f.geometry, 1, null);
    expect(f.camera.read()?.camera.scale).toBeCloseTo(displayed.camera.scale);
    f.camera.dispose();
    const count = f.shown.length;
    f.clock.advance(100);
    f.camera.show(slot, f.geometry, 1, null);
    expect(f.camera.zoom(2)).toBe(false);
    f.camera.dispose();
    expect(f.shown).toHaveLength(count);
    expect(f.clock.pending).toBe(0);
    f.inspections.dispose();
  });

  test('runs continuous pinch/pan without inertia and routes double click through the same camera', () => {
    const f = fixture(true);
    const slot = f.slots[0]!;
    f.camera.show(slot, f.geometry, 1, null);
    f.camera.doubleClick({ x: 400, y: 300 });
    const before = f.camera.read()!.camera;
    f.camera.pinch(1, { x: 400, y: 300 }, { x: -20, y: 10 });
    expect(f.camera.read()?.camera).toMatchObject({ x: before.x - 20, y: before.y + 10 });
    expect(f.clock.pending).toBe(0);
    f.camera.doubleClick({ x: 400, y: 300 });
    expect(f.camera.read()?.camera.mode).toBe('fit');
    f.camera.wheel(-0.5, 0, { x: 400, y: 300 });
    expect(f.camera.read()?.camera.scale).toBeGreaterThan(
      f.geometry.viewport.width / f.geometry.image.width,
    );
    expect(f.clock.pending).toBe(0);
    f.camera.dispose();
    f.inspections.dispose();
  });
});

test('independent cameras survive switching same-source batches across prompts of one scene run', () => {
  const f = fixture(true);
  const source = f.slots[0];
  if (!source) throw new Error('Missing camera source.');
  const a = { ...source, scene: ['scene-run', 'A'] as const };
  const b = { ...source, scene: ['scene-run', 'B'] as const };
  f.inspections.synchronize(f.view, f.catalog.generation, [a, b]);
  f.inspections.setLinked(f.view, false, null);
  f.camera.show(a, f.geometry, 1, null);
  f.camera.native();
  const first = f.camera.read()?.camera;
  f.camera.show(b, f.geometry, 2, null);
  expect(f.camera.read()?.camera.mode).toBe('fit');
  f.camera.zoom(3);
  const second = f.camera.read()?.camera;
  f.camera.show(a, f.geometry, 1, null);
  expect(f.camera.read()?.camera).toEqual(first);
  f.camera.show(b, f.geometry, 2, null);
  expect(f.camera.read()?.camera).toEqual(second);
  expect(f.inspections.snapshot(f.view).individual).toHaveLength(2);
  f.camera.dispose();
});

test.each([
  ['custom', 32],
  ['native', 1],
] as const)(
  'restores an off-image %s camera exactly before presentation and after density resize',
  (mode, physicalScale) => {
    const f = fixture(true);
    const slot = f.slots[0];
    if (!slot) throw new Error('Missing saved camera slot.');
    const saved = {
      mode,
      region: {
        centerX: -3.25,
        centerY: 2.75,
        spanX: 1 / (f.geometry.image.width * physicalScale),
        spanY: 1 / (f.geometry.image.height * physicalScale),
      },
    };
    f.inspections.presentRun(f.view, 1);
    f.inspections.record(f.view, slot, saved);
    f.camera.show(slot, f.geometry, 1, null);
    const first = f.camera.read();
    if (!first) throw new Error('Saved camera was not presented.');
    expect(first.camera.mode).toBe(mode);
    expect(first.camera.scale * f.geometry.density).toBeCloseTo(physicalScale);
    const displayed = captureCanvasInspection(first.camera, first.geometry);
    expect(displayed.region.centerX).toBeCloseTo(saved.region.centerX);
    expect(displayed.region.centerY).toBeCloseTo(saved.region.centerY);
    const resized = canvasCameraGeometry(f.geometry.image, { width: 1200, height: 700 }, 2);
    if (!resized) throw new Error('Missing resized camera geometry.');
    f.camera.show(slot, resized, 1, null);
    const after = f.camera.read();
    if (!after) throw new Error('Resized camera was not presented.');
    expect(after.camera.mode).toBe(mode);
    expect(after.camera.scale * resized.density).toBeCloseTo(physicalScale);
    const captured = captureCanvasInspection(after.camera, resized);
    expect(captured.region.centerX).toBeCloseTo(saved.region.centerX);
    expect(captured.region.centerY).toBeCloseTo(saved.region.centerY);
    expect(captured.region.spanX).toBeCloseTo(saved.region.spanX, 12);
    expect(captured.region.spanY).toBeCloseTo(saved.region.spanY, 12);
    f.camera.dispose();
  },
);
