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
/** Preserve shared and independent final inspections across DTO-style snapshot import and geometry changes. */
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasViewId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import {
  captureCanvasInspection,
  projectCanvasInspection,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasInspection.js';

test('linked snapshots retain independent cameras and an out-of-image shared center', () => {
  const store = new CanvasInspectionStore();
  const view = canvasViewId('old');
  const slot = { sourceId: outputSourceId('1'), batchIndex: 0 };
  const individual = {
    mode: 'native' as const,
    region: { centerX: 0.3, centerY: 0.7, spanX: 0.001, spanY: 0.002 },
  };
  const shared = {
    mode: 'custom' as const,
    region: { centerX: -0.5, centerY: 1.5, spanX: 0.004, spanY: 0.004 },
  };
  store.synchronize(view, 3, [slot]);
  store.presentRun(view, 7);
  store.setLinked(view, false, null);
  store.record(view, slot, individual);
  store.setLinked(view, true, null);
  store.record(view, slot, shared);
  const snapshot = JSON.parse(JSON.stringify(store.snapshot(view)));
  const fresh = canvasViewId('fresh');
  store.restore(fresh, 1, snapshot, [slot]);
  expect(store.inspection(fresh, slot)).toEqual(shared);
  expect(store.presentRun(fresh, 7)).toBe(false);
  store.setLinked(fresh, false, null);
  expect(store.inspection(fresh, slot)).toEqual(individual);
  expect(store.snapshot(view).shared?.region.centerX).toBe(-0.5);
  expect(store.snapshot(view).shared?.region.centerY).toBe(1.5);
});

test('restored custom inspection preserves physical magnification across viewport and density changes', () => {
  const before = {
    viewport: { width: 800, height: 600 },
    image: { width: 1600, height: 1200 },
    density: 1,
  };
  const after = { ...before, viewport: { width: 1200, height: 700 }, density: 2 };
  const inspection = captureCanvasInspection(
    { x: -1200, y: -400, scale: 2, mode: 'custom' },
    before,
  );
  const projected = projectCanvasInspection(inspection, after);
  expect(projected.scale * after.density).toBeCloseTo(2);
  const roundtrip = captureCanvasInspection(projected, after);
  expect(roundtrip.region.spanX).toBeCloseTo(inspection.region.spanX);
  expect(roundtrip.region.spanY).toBeCloseTo(inspection.region.spanY);
});

test('invalid or removed slots fail before replacing an existing retained inspection', () => {
  const store = new CanvasInspectionStore();
  const view = canvasViewId('view');
  const slot = { sourceId: outputSourceId('1'), batchIndex: 0 };
  store.synchronize(view, 0, [slot]);
  const before = store.snapshot(view);
  expect(() =>
    store.restore(
      view,
      1,
      {
        linked: false,
        shared: null,
        presentedRun: 1,
        individual: [
          {
            selection: slot,
            inspection: { mode: 'custom', region: { centerX: 0, centerY: 0, spanX: 0, spanY: 1 } },
          },
        ],
      },
      [slot],
    ),
  ).toThrow('invalid');
  expect(store.snapshot(view)).toEqual(before);
});

test('settled final inspection changes notify once while repeated echoes and streaming previews do not', () => {
  const store = new CanvasInspectionStore();
  const view = canvasViewId('observed');
  const slot = { sourceId: outputSourceId('1'), batchIndex: 0 };
  store.synchronize(view, 0, [slot]);
  let changes = 0;
  store.subscribe(() => changes++);
  const inspection = {
    mode: 'custom' as const,
    region: { centerX: 0.5, centerY: 0.5, spanX: 0.001, spanY: 0.001 },
  };
  store.record(view, slot, inspection);
  store.record(view, slot, inspection);
  expect(changes).toBe(1);
  store.capture(
    view,
    slot,
    { mode: 'fit', scale: 1, x: 0, y: 0 },
    { image: { width: 100, height: 100 }, viewport: { width: 100, height: 100 }, density: 1 },
    'live-preview',
  );
  expect(changes).toBe(1);
});

test('numerically unrepresentable inspection refuses restoration instead of silently selecting Fit', () => {
  const geometry = {
    image: { width: 100, height: 100 },
    viewport: { width: 800, height: 600 },
    density: 1,
  };
  expect(() =>
    projectCanvasInspection(
      {
        mode: 'custom',
        region: {
          centerX: Number.MAX_VALUE,
          centerY: 0.5,
          spanX: Number.MIN_VALUE,
          spanY: Number.MIN_VALUE,
        },
      },
      geometry,
    ),
  ).toThrow('cannot be represented');
});
