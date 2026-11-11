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
/** Prove the native snapshot locks the viewer controls only while preview pixels are shown. */
import { expect, test } from '@jest/globals';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewProjection } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewProjection.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { nativeCanvasSnapshot } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasSnapshot.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

/** Project one committed view of the fixture catalog with an accepted Output live preview. */
function snapshot(input: boolean) {
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-snapshot'));
  const plan = new CanvasViewProjection(
    view,
    views,
    selection,
    new CanvasInspectionStore(),
  ).project(catalog);
  return nativeCanvasSnapshot({
    address,
    plan,
    catalog,
    sources: catalog.entries.map((entry) => entry.summary),
    connected: true,
    mediaActions: null,
    presentation: { state: 'ready', message: '' },
    viewport: null,
    preview: true,
    previewAvailable: true,
    comparison: false,
    overview: false,
    input,
  });
}

test('an Output live preview locks the viewer controls only while the Output route shows it', () => {
  expect(snapshot(false).preview).toBe(true);
  expect(snapshot(true).preview).toBe(false);
});
