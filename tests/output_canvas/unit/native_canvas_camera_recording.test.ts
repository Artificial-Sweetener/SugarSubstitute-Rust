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
/** Prove native camera recording semantics against the shared inspection owner. */
import { expect, test } from '@jest/globals';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import {
  canvasSurfaceInspection,
  nativeCanvasViewportInspection,
  NativeCanvasInspection,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasInspection.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import type { CanvasSurfaceViewportState } from '../../../integrations/contracts/CanvasSurface.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

const zoomed: CanvasSurfaceViewportState = {
  mode: 'custom',
  physicalScale: 2,
  dimensions: { width: 3840, height: 2160 },
  center: { x: 0.25, y: 0.75 },
};
const expected = {
  center: { x: 0.25, y: 0.75 },
  span: { x: 1 / 7680, y: 1 / 4320 },
};

/** Build real view, selection and inspection owners around one native inspection bridge. */
function fixture(enabled = true) {
  const views = new CanvasViewRegistry(() => 'key');
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  const inspections = new CanvasInspectionStore();
  const catalog: CanvasCatalogSnapshot = canvasCatalogFixture(1, 0, undefined, 'unit');
  const view = views.create(catalog.sessionId).viewId;
  selection.reconcile(view, catalog);
  inspections.synchronize(
    view,
    catalog.generation,
    catalog.entries.map((entry) => ({ sourceId: entry.source.id, batchIndex: 0 })),
  );
  const commands: unknown[] = [];
  const bridge = new NativeCanvasInspection({
    view,
    inspections,
    enabled,
    viewport: (command) => {
      commands.push(command);
      return Promise.resolve(true);
    },
    accepts: () => true,
  });
  /** Show one source's output through the shared selection owner. */
  const show = (index: number) => {
    const source = catalog.entries[index]?.source;
    if (!source) throw new Error('Missing source.');
    selection.apply(view, catalog, {
      type: 'select',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    const plan = canvasDisplayPlan(views.read(view), catalog, selection.items(view, catalog), null);
    const item = plan.primary?.item;
    if (!item) throw new Error('Missing item.');
    return { plan, item };
  };
  return { views, view, catalog, selection, inspections, bridge, commands, show };
}

test('translates a settled host camera into the shared inspection and back to the wire', () => {
  const inspection = nativeCanvasViewportInspection(zoomed);
  expect(inspection).toEqual({
    mode: 'custom',
    region: { centerX: 0.25, centerY: 0.75, spanX: 1 / 7680, spanY: 1 / 4320 },
  });
  expect(canvasSurfaceInspection(inspection)).toEqual(expected);
  const { center: _center, ...centerless } = zoomed;
  expect(nativeCanvasViewportInspection(centerless)).toBeNull();
  expect(canvasSurfaceInspection(null)).toBeNull();
  const region = { centerX: 0.5, centerY: 0.5, spanX: 0.001, spanY: 0.001 };
  expect(canvasSurfaceInspection({ mode: 'fit', region })).toBeNull();
  expect(
    canvasSurfaceInspection({ mode: 'native', region: { ...region, centerX: -0.2, centerY: 1.4 } }),
  ).toEqual({ center: { x: 0, y: 1 }, span: { x: 0.001, y: 0.001 } });
  for (const span of [0, -1, 1_000_000_001, Number.NaN, Number.POSITIVE_INFINITY])
    expect(
      canvasSurfaceInspection({ mode: 'custom', region: { ...region, spanY: span } }),
    ).toBeNull();
  expect(
    canvasSurfaceInspection({ mode: 'custom', region: { ...region, centerX: Number.NaN } }),
  ).toBeNull();
});

test('previews, overview grids and another image size never replace the saved camera', () => {
  const f = fixture();
  const { plan, item } = f.show(0);
  f.bridge.observe(plan, zoomed, false);
  const saved = f.inspections.inspection(f.view, {
    sourceId: item.provenance.sourceId,
    batchIndex: 0,
  });
  const other: CanvasSurfaceViewportState = { ...zoomed, center: { x: 0.9, y: 0.1 } };
  f.bridge.observe(plan, other, true);
  f.bridge.observe({ ...plan, view: { ...plan.view, mode: 'overview' } }, other, false);
  f.bridge.observe(plan, { ...other, dimensions: { width: 19, height: 17 } }, false);
  f.bridge.observe(plan, null, false);
  expect(
    f.inspections.inspection(f.view, { sourceId: item.provenance.sourceId, batchIndex: 0 }),
  ).toBe(saved);
  expect(f.bridge.restore(f.show(1).item, false)).toEqual(expected);
});

test('comparison partners keep the presented camera instead of a slot camera', () => {
  const f = fixture();
  f.views.update(f.view, (state) => ({ ...state, linked: false }));
  const first = f.show(0);
  f.bridge.link(first.plan);
  f.bridge.observe(first.plan, zoomed, false);
  const second = f.show(1);
  expect(f.bridge.restore(second.item, false)).toBeNull();
  expect(f.bridge.restore(second.item, true)).toEqual(expected);
});

test('the first presented Run starts at Fit and a later Run retires saved cameras', () => {
  const f = fixture();
  const { plan, item } = f.show(0);
  expect(f.bridge.restore(item, false)).toBeNull();
  f.bridge.observe(plan, zoomed, false);
  expect(f.bridge.restore(item, false)).toEqual(expected);
  expect(f.inspections.retires(f.view, 1)).toBe(false);
  expect(f.inspections.retires(f.view, 2)).toBe(true);
  const later = { ...item, provenance: { ...item.provenance, runOrder: 2 } };
  expect(f.bridge.restore(later, false)).toBeNull();
  expect(f.bridge.restore(item, false)).toEqual(expected);
});

test('a host without inspection records and restores nothing', () => {
  const f = fixture(false);
  const { plan, item } = f.show(0);
  f.bridge.observe(plan, zoomed, false);
  expect(
    f.inspections.inspection(f.view, { sourceId: item.provenance.sourceId, batchIndex: 0 }),
  ).toBeNull();
  expect(f.bridge.restore(item, false)).toBeNull();
  f.views.update(f.view, (state) => ({ ...state, linked: false }));
  f.bridge.link(f.show(0).plan);
  expect(f.commands).toEqual([]);
});

test('link changes re-inspect only a shown final and withdraw a superseded request', () => {
  const f = fixture();
  const first = f.show(0);
  f.views.update(f.view, (state) => ({ ...state, linked: false }));
  f.bridge.link(f.show(0).plan);
  expect(f.commands).toEqual([]);
  f.bridge.observe(first.plan, zoomed, false);
  f.views.update(f.view, (state) => ({ ...state, linked: true }));
  f.bridge.link(f.show(0).plan);
  expect(f.commands).toEqual([{ type: 'inspect', inspection: expected }]);
  f.bridge.dispose();
});
