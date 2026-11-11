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
/** Prove result selection never transfers workflow input files to the output presenter. */
import {
  nativeCanvasDesired,
  nativeCanvasSelection,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasDesired.js';
import { desktopOutputResultId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';
import { previewGeometry } from '../support/NativeCanvasPreviewFixture.js';

const output = canvasCatalogFixture().entries[0]!.items[0]!;
const input = { ...output, file: { ...output.file, type: 'input' as const } };

test('an input thumbnail is not an output selection', () => {
  const desired = nativeCanvasDesired(previewGeometry, input, null, null, true, false, false);
  expect(nativeCanvasSelection(desired)).toBeNull();
});

test('an input thumbnail is not offered as the output comparison', () => {
  const desired = nativeCanvasDesired(
    previewGeometry,
    output,
    { item: input, orientation: 'vertical', split: 0.32 },
    null,
    true,
    false,
    false,
  );
  expect(nativeCanvasSelection(desired)?.operation).toEqual({
    type: 'surface.select',
    itemId: output.id,
    file: output.file,
  });
});

test.each(['output', 'temp'] as const)('a genuine %s image remains selectable', (type) => {
  const item = { ...output, file: { ...output.file, type } };
  const desired = nativeCanvasDesired(previewGeometry, item, null, null, true, false, false);
  expect(nativeCanvasSelection(desired)?.operation).toEqual({
    type: 'surface.select',
    itemId: item.id,
    file: item.file,
  });
});

test('the Input route presents its native document without selecting catalog outputs', () => {
  const desired = nativeCanvasDesired(
    previewGeometry,
    output,
    { item: output, orientation: 'vertical', split: 0.32 },
    [{ itemId: output.id, file: output.file, rect: { x: 0, y: 0, width: 100, height: 100 } }],
    true,
    false,
    true,
  );
  expect(desired).toMatchObject({
    shown: true,
    empty: false,
    previewable: false,
    item: null,
    comparison: null,
    overview: null,
  });
  expect(nativeCanvasSelection(desired)).toBeNull();
  const outputDesired = nativeCanvasDesired(
    previewGeometry,
    output,
    null,
    null,
    true,
    false,
    false,
  );
  expect(desired.content).not.toBe(outputDesired.content);
});

test('a restored desktop result and its comparison are selected under the host contract identity', () => {
  const restored = { ...output, id: desktopOutputResultId('3e0f2c6b-7a84-4691-9d4f-9730420bd2ca') };
  const partner = { ...output, id: desktopOutputResultId('f40b2275-22f4-4e44-a199-32a8bba728f2') };
  const desired = nativeCanvasDesired(
    previewGeometry,
    restored,
    { item: partner, orientation: 'vertical', split: 0.5 },
    null,
    true,
    false,
    false,
  );
  const operation = nativeCanvasSelection(desired)?.operation;
  expect(operation).toMatchObject({
    type: 'surface.select',
    itemId: `result:${restored.id}`,
    comparison: { itemId: `result:${partner.id}` },
  });
  expect(
    isCanvasMessage({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session: 'panel-0123456789abcdef',
      requestId: 1,
      address: canvasSnapshot('restored').address,
      revision: 1,
      ...operation,
    }),
  ).toBe(true);
});
