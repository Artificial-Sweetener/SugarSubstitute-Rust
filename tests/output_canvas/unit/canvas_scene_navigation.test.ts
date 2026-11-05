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
/** Prove the output scene picker's All overview and per-scene navigation, as in Substitute. */

import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { canvasSceneCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import {
  canvasOverviewSceneCommand,
  canvasOverviewWindow,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOverviewWindow.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import type { WorkflowOutputSet } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import { canvasCatalogFixture, canvasSources } from '../support/CanvasCatalogFixture.js';

/** Give each scene its own run's copy of the fixture outputs; `empty` scenes have none yet. */
function sceneResult(empty: readonly string[] = []): WorkflowResultSession {
  const base = canvasCatalogFixture(1).result;
  const scenes = [
    { key: 'morning', title: 'Morning', order: 0 },
    { key: 'night', title: 'Night', order: 1 },
    { key: 'dusk', title: 'Dusk', order: 2 },
  ];
  const outputs = (scene: string, run: number): WorkflowOutputSet[] =>
    base.outputs.map((output) => ({
      ...output,
      items: output.items.map((item) => ({
        ...item,
        id: `${item.id}:${scene}` as typeof item.id,
        provenance: { ...item.provenance, runOrder: run },
        file: { ...item.file, filename: `${scene}.png` },
      })),
    }));
  return {
    ...base,
    runOrder: 3,
    scenes: {
      id: 'scene-run',
      running: 'dusk',
      scenes: scenes.map((scene, index) => ({
        ...scene,
        outputs: empty.includes(scene.key) ? [] : outputs(scene.key, index + 1),
      })),
    },
  };
}

/** Create one view around the scene results. */
function fixture(result = sceneResult()) {
  let identity = 0;
  const views = new CanvasViewRegistry(() => String(++identity));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const view = views.create(
    canvasSceneCatalog(result, canvasSources(), null, null).sessionId,
  ).viewId;
  const catalog = () => canvasSceneCatalog(result, canvasSources(), null, views.read(view).scene);
  return { views, selection, view, catalog };
}

test('every scene together shows one representative image per scene', () => {
  const catalog = canvasSceneCatalog(sceneResult(['dusk']), canvasSources(), null, null);
  expect(catalog.sceneOverview).toBe(true);
  expect(catalog.scenes).toEqual([
    { key: 'morning', title: 'Morning', available: true },
    { key: 'night', title: 'Night', available: true },
    { key: 'dusk', title: 'Dusk', available: false },
  ]);
  expect(catalog.entries.map((entry) => [entry.scene, entry.items[0]?.file.filename])).toEqual([
    ['morning', 'morning.png'],
    ['night', 'night.png'],
  ]);
  const window = canvasOverviewWindow(catalog, 800, 600, 0);
  expect(window.cells.map((cell) => cell.label)).toEqual(['Morning', 'Night']);
  expect(new Set(window.cells.map((cell) => cell.key)).size).toBe(2);
  expect(canvasOverviewSceneCommand(window.cells[1]!)).toEqual({
    type: 'select-scene',
    scene: 'night',
  });
});

test('a chosen scene shows only its own outputs as sources', () => {
  const catalog = canvasSceneCatalog(sceneResult(), canvasSources(), null, 'night');
  expect(catalog.sceneOverview).toBe(false);
  expect(catalog.entries).toHaveLength(3);
  expect(catalog.entries.every((entry) => entry.items[0]?.file.filename === 'night.png')).toBe(
    true,
  );
});

test('a scene run opens on All; choosing a scene opens it and All returns to the overview', () => {
  const f = fixture();
  f.selection.reconcile(f.view, f.catalog());
  expect(f.views.read(f.view)).toMatchObject({ scene: null, mode: 'overview' });
  const plan = canvasDisplayPlan(
    f.views.read(f.view),
    f.catalog(),
    { primary: null, secondary: null },
    null,
  );
  expect(plan.grid.map((entry) => entry.scene)).toEqual(['morning', 'night', 'dusk']);
  f.selection.apply(f.view, f.catalog(), { type: 'select-scene', scene: 'night' });
  expect(f.views.read(f.view)).toMatchObject({
    scene: 'night',
    // The fixture's source holds a batch of three, so the scene opens on its batch grid.
    mode: 'overview',
    follow: 'manual',
    selection: { sourceId: canvasSources()[0]!.id, batchIndex: 0 },
  });
  f.selection.apply(f.view, f.catalog(), { type: 'select-scene', scene: null });
  expect(f.views.read(f.view)).toMatchObject({ scene: null, mode: 'overview' });
});

test('a scene with nothing to show cannot be chosen', () => {
  const f = fixture(sceneResult(['dusk']));
  f.selection.reconcile(f.view, f.catalog());
  f.selection.apply(f.view, f.catalog(), { type: 'select-scene', scene: 'dusk' });
  expect(f.views.read(f.view).scene).toBeNull();
});
