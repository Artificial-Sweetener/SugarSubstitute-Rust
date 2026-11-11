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
/** Prove image comparison cannot trap navigation to playable media. */
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { mixedMediaFixture } from '../support/CanvasMixedMediaFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

test('switches from a later mixed batch slot to an available item in another source', () => {
  const { catalog: mixed, items } = mixedMediaFixture();
  const destination = canvasCatalogFixture().entries[1]!;
  const catalog = { ...mixed, entries: [...mixed.entries, destination] };
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.apply(view, catalog, {
    type: 'select',
    selection: { sourceId: items[2]!.provenance.sourceId, batchIndex: 2 },
  });
  selection.apply(view, catalog, {
    type: 'select-source',
    selection: { sourceId: destination.source.id, batchIndex: 2 },
  });
  expect(views.read(view).selection).toEqual({
    sourceId: destination.source.id,
    batchIndex: destination.items[0]!.batchIndex,
  });
});

test('leaves image comparison when the user selects video or a model', () => {
  const { catalog, items } = mixedMediaFixture();
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const image = { sourceId: items[0]!.provenance.sourceId, batchIndex: 0 };
  for (const index of [1, 2]) {
    views.update(view, (state) => ({
      ...state,
      follow: 'manual',
      mode: 'image',
      sourcePreview: false,
      selection: image,
      comparison: { secondary: image, orientation: 'vertical', split: 0.5 },
    }));
    selection.apply(view, catalog, { type: 'select', selection: { ...image, batchIndex: index } });
    expect(views.read(view).comparison).toBeNull();
    expect(views.read(view).selection?.batchIndex).toBe(index);
  }
});
