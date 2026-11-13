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
/** Prove catalog presentation changes preserve inspection for retained workflow image slots. */

import { CanvasCameraController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasCameraController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewProjection } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewProjection.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import {
  canvasCameraGeometry,
  type CanvasCamera,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Compose real projection, selection and camera owners with deterministic presentation time. */
function fixture() {
  const initial = canvasCatalogFixture();
  const first = initial.result.outputs[0]!;
  const extra: WorkflowOutputItem = { ...first.items[0]!, id: 'result:extra-batch', batchIndex: 1 };
  const result = {
    ...initial.result,
    outputs: [{ ...first, items: [...first.items, extra] }, ...initial.result.outputs.slice(1)],
  };
  const catalog = canvasOutputCatalog(result, initial.sources);
  const views = new CanvasViewRegistry(() => 'projection');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const inspections = new CanvasInspectionStore();
  const projection = new CanvasViewProjection(view, views, selection, inspections);
  projection.project(catalog);
  const geometry = canvasCameraGeometry(
    { width: 3840, height: 2160 },
    { width: 800, height: 600 },
    1,
  )!;
  const camera = new CanvasCameraController({
    view,
    inspections,
    clock: new CanvasFrameFixture(),
    reducedMotion: () => true,
    present: () => {},
  });
  camera.show({ sourceId: first.source.id, batchIndex: 0 }, geometry, 1, null);
  camera.link(false);
  const retained = [extra, result.outputs[1]!.items[0]!].map((item, index) => {
    const slot = { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex };
    camera.show(slot, geometry, 1, null);
    camera.native();
    camera.zoom(index + 2);
    camera.pan({ x: -50 * (index + 1), y: 30 });
    return { item, slot, camera: camera.read()!.camera };
  });
  const preview = {
    sessionId: result.context.sessionId,
    generation: result.context.generation,
    promptId: 'prompt:next' as const,
    sourceId: first.source.id,
    runOrder: 2,
    sequence: 1,
    token: 'preview:next',
    dimensions: { width: 512, height: 288 },
  };
  const previewResult = { ...result, runOrder: 2, presentedRunOrder: 2, preview };
  return {
    catalog,
    result,
    views,
    view,
    selection,
    inspections,
    projection,
    camera,
    geometry,
    retained,
    preview,
    previewResult,
  };
}

/** Compare durable camera projection with tolerance for image-relative coordinate roundoff. */
function expectCamera(actual: CanvasCamera, expected: CanvasCamera): void {
  expect(actual.mode).toBe(expected.mode);
  expect(actual.scale).toBeCloseTo(expected.scale);
  expect(actual.x).toBeCloseTo(expected.x);
  expect(actual.y).toBeCloseTo(expected.y);
}

test('keeps hidden unlinked batch and source cameras through preview projection and exact final reveals', () => {
  const f = fixture();
  const catalog = canvasOutputCatalog(f.previewResult, f.catalog.sources);
  expect(catalog.entries.flatMap((entry) => entry.items)).toHaveLength(0);
  f.selection.reconcile(f.view, catalog);
  f.projection.project(catalog);
  const previewGeometry = canvasCameraGeometry(f.preview.dimensions, f.geometry.viewport, 1)!;
  f.camera.show(
    { sourceId: f.preview.sourceId, batchIndex: 0 },
    previewGeometry,
    2,
    'preview:next',
  );
  expect(f.camera.read()?.camera.mode).toBe('fit');
  expect(f.camera.zoom(2)).toBe(false);
  for (const retained of f.retained) {
    expectCamera(f.inspections.camera(f.view, retained.slot, f.geometry, null), retained.camera);
    expect(
      f.selection.apply(f.view, catalog, {
        type: 'reveal',
        target: canvasRevealTarget(retained.item),
      }),
    ).toBe(true);
    const inspected = canvasOutputCatalog(
      f.previewResult,
      catalog.sources,
      f.selection.inspection(f.view),
    );
    f.projection.project(inspected);
    f.camera.show(retained.slot, f.geometry, 1, null);
    expectCamera(f.camera.read()!.camera, retained.camera);
  }
  f.camera.dispose();
  f.inspections.dispose();
});

test('prunes removed sources even while their final descriptors remain retained in results', () => {
  const f = fixture();
  const removed = f.retained[1]!;
  const sources = f.catalog.sources.filter((source) => source.id !== removed.slot.sourceId);
  const catalog = canvasOutputCatalog(f.previewResult, sources);
  f.selection.reconcile(f.view, catalog);
  f.projection.project(catalog);
  expect(f.inspections.camera(f.view, removed.slot, f.geometry, null).mode).toBe('fit');
  const available = f.retained[0]!;
  expectCamera(f.inspections.camera(f.view, available.slot, f.geometry, null), available.camera);
  f.camera.dispose();
  f.inspections.dispose();
});

test('retires preserved independent cameras when the newer final is actually presented', () => {
  const f = fixture();
  const previewCatalog = canvasOutputCatalog(f.previewResult, f.catalog.sources);
  f.selection.reconcile(f.view, previewCatalog);
  f.projection.project(previewCatalog);
  const nextBase = canvasCatalogFixture(2, 0, f.catalog.sources);
  const first = nextBase.result.outputs[0]!;
  const extra: WorkflowOutputItem = {
    ...first.items[0]!,
    id: 'result:next-extra-batch',
    batchIndex: 1,
  };
  const next = canvasOutputCatalog(
    {
      ...nextBase.result,
      outputs: [{ ...first, items: [...first.items, extra] }, ...nextBase.result.outputs.slice(1)],
    },
    nextBase.sources,
  );
  f.selection.reconcile(f.view, next);
  f.projection.project(next);
  const retained = f.retained[1]!;
  expectCamera(f.inspections.camera(f.view, retained.slot, f.geometry, null), retained.camera);
  const other = f.retained[0]!;
  expectCamera(f.inspections.camera(f.view, other.slot, f.geometry, null), other.camera);
  f.camera.show(retained.slot, f.geometry, 2, null);
  expect(f.camera.read()?.camera.mode).toBe('fit');
  expect(f.inspections.camera(f.view, f.retained[0]!.slot, f.geometry, null).mode).toBe('fit');
  f.camera.dispose();
  f.inspections.dispose();
});
